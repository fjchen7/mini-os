//! 块缓存管理模块

use crate::{block_dev::BlockDevice, BLOCK_SZ};
use alloc::{collections::vec_deque::VecDeque, sync::Arc};
use lazy_static::*;
use spin::Mutex;

pub struct BlockCache {
    // 要缓存的块数据
    cache: [u8; BLOCK_SZ],
    // 该缓存对应的块号
    block_id: usize,
    // 缓存的块设备，可通过它读写块
    block_device: Arc<dyn BlockDevice>,
    // 自该快被缓存后，是否被修改过（脏位，dirty）
    modified: bool,
}

impl BlockCache {
    // 从磁盘加载一个新的块缓存
    pub fn new(block_id: usize, block_device: Arc<dyn BlockDevice>) -> Self {
        let mut cache = [0u8; BLOCK_SZ];
        block_device.read_block(block_id, &mut cache);
        Self {
            cache,
            block_id,
            block_device,
            modified: false,
        }
    }

    // 得到缓存块数据中，某个偏移量的地址
    fn addr_of_offset(&self, offset: usize) -> usize {
        &self.cache[offset] as *const _ as usize
    }

    // 将缓存区里偏移量地址开始的一段连续数据，解析成T类型，并返回其引用
    pub fn get_ref<T>(&self, offset: usize) -> &T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        let addr = self.addr_of_offset(offset);
        unsafe { &*(addr as *const T) }
    }

    // get_ref的可变版本
    pub fn get_mut<T>(&mut self, offset: usize) -> &mut T
    where
        T: Sized,
    {
        let type_size = core::mem::size_of::<T>();
        assert!(offset + type_size <= BLOCK_SZ);
        self.modified = true;
        let addr = self.addr_of_offset(offset);
        unsafe { &mut *(addr as *mut T) }
    }

    pub fn read<T, V>(&self, offset: usize, f: impl FnOnce(&T) -> V) -> V {
        f(self.get_ref(offset))
    }

    pub fn modify<T, V>(&mut self, offset: usize, f: impl FnOnce(&mut T) -> V) -> V {
        f(self.get_mut(offset))
    }

    // 将缓存写回磁盘
    pub fn sync(&mut self) {
        if self.modified {
            self.modified = false;
            self.block_device.write_block(self.block_id, &self.cache);
        }
    }
}

impl Drop for BlockCache {
    fn drop(&mut self) {
        self.sync()
    }
}

// 内存中最多缓存16个块
const BLOCK_CACHE_SIZE: usize = 16;

// 块缓存管理器
pub struct BlockCacheManager {
    // 缓存队列，每个元素表示(块号，块缓存)
    queue: VecDeque<(usize, Arc<Mutex<BlockCache>>)>,
}

impl BlockCacheManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    // 从存储设备中读取一个块，并进行缓存。
    // 如果该块已经被缓存，则直接返回。
    pub fn get_block_cache(
        &mut self,
        block_id: usize,
        block_device: Arc<dyn BlockDevice>,
    ) -> Arc<Mutex<BlockCache>> {
        if let Some(pair) = self.queue.iter().find(|pair| pair.0 == block_id) {
            Arc::clone(&pair.1)
        } else {
            // 如果缓存队列已满，则删除一个缓存块
            if self.queue.len() == BLOCK_CACHE_SIZE {
                // 类似FIFO算法，从队头开始，找到没有在其他地方被引用的缓存块，然后删除
                if let Some((idx, _)) = self
                    .queue
                    .iter()
                    .enumerate()
                    .find(|(_, pair)| Arc::strong_count(&pair.1) == 1)
                {
                    self.queue.drain(idx..=idx);
                } else {
                    panic!("Run out of BlockCache!");
                }
            }
            // 从磁盘加载块数据，并创建一个新的块缓存
            let block_cache = Arc::new(Mutex::new(BlockCache::new(
                block_id,
                Arc::clone(&block_device),
            )));
            self.queue.push_back((block_id, Arc::clone(&block_cache)));
            block_cache
        }
    }
}

lazy_static! {
    // 全局的块缓存管理器。由于可能被多个线程访问，因此需要Mutex。
    pub static ref BLOCK_CACHE_MANAGER: Mutex<BlockCacheManager> =
        Mutex::new(BlockCacheManager::new());
}

// 拿到给定块号和块设备对应的块缓存
pub fn get_block_cache(
    block_id: usize,
    block_device: Arc<dyn BlockDevice>,
) -> Arc<Mutex<BlockCache>> {
    BLOCK_CACHE_MANAGER
        .lock()
        .get_block_cache(block_id, block_device)
}

// 将所有块缓存写回磁盘
pub fn block_cache_sync_all() {
    let manager = BLOCK_CACHE_MANAGER.lock();
    for (_, cache) in manager.queue.iter() {
        cache.lock().sync();
    }
}
