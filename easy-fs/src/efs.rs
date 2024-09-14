//! 磁盘快管理器

use crate::{
    bitmap::Bitmap,
    block_cache::{block_cache_sync_all, get_block_cache},
    block_dev::BlockDevice,
    layout::{DiskInode, DiskInodeType, SuperBlock},
    vfs::Inode,
    BLOCK_SZ,
};
use alloc::sync::Arc;
use spin::Mutex;
///An easy file system on block
pub struct EasyFileSystem {
    // 块设备（如磁盘）
    pub block_device: Arc<dyn BlockDevice>,
    // 记录inode的使用情况
    pub inode_bitmap: Bitmap,
    // 记录数据块的使用情况
    pub data_bitmap: Bitmap,
    inode_area_start_block: u32,
    data_area_start_block: u32,
}

type DataBlock = [u8; BLOCK_SZ];

impl EasyFileSystem {
    // 初始化块设备，新建文件系统
    pub fn create(
        block_device: Arc<dyn BlockDevice>,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
    ) -> Arc<Mutex<Self>> {
        // 共inode_bitmap_blocks个块作为inode位图
        let inode_bitmap = Bitmap::new(1, inode_bitmap_blocks as usize);
        // 该inode位图最多能表示的inode编号
        let inode_num = inode_bitmap.maximum();
        // 存放这么多inode结构需要的块数，要向上取整
        let inode_area_blocks = {
            let bytes = inode_num * core::mem::size_of::<DiskInode>();
            bytes.div_ceil(BLOCK_SZ) as u32
        };
        // 存放inode位图和inode数据类型的块的总数
        let inode_total_blocks = inode_bitmap_blocks + inode_area_blocks;
        // 存放数据位图和数据的块数。-1是为了留出超级块的位置。
        let data_total_blocks = total_blocks - 1 - inode_total_blocks;
        // 一个块的数据位图可表示4096个数据块的使用情况，所以1+4096为一组
        // 因此数据位图块的数量的计算方式为：剩余块数除以4097，再向上取整
        let data_bitmap_blocks = (data_total_blocks + (4097 - 1)) / 4097;
        // 存放数据的块数
        let data_area_blocks = data_total_blocks - data_bitmap_blocks;
        // 数据位图块前面，是超级块、inode位图块、inode数据块
        let data_bitmap = Bitmap::new(
            (1 + inode_bitmap_blocks + inode_area_blocks) as usize,
            data_bitmap_blocks as usize,
        );
        let mut efs = Self {
            block_device: Arc::clone(&block_device),
            inode_bitmap,
            data_bitmap,
            inode_area_start_block: 1 + inode_bitmap_blocks,
            data_area_start_block: 1 + inode_total_blocks + data_bitmap_blocks,
        };
        // 初始化块设备，将所有块清零
        for i in 0..total_blocks {
            get_block_cache(i as usize, Arc::clone(&block_device))
                .lock()
                .modify(0, |data_block: &mut DataBlock| {
                    for byte in data_block.iter_mut() {
                        *byte = 0;
                    }
                });
        }
        // 初始化超级块，用于存放文件系统的元信息
        get_block_cache(0, Arc::clone(&block_device)).lock().modify(
            0,
            |super_block: &mut SuperBlock| {
                super_block.initialize(
                    total_blocks,
                    inode_bitmap_blocks,
                    inode_area_blocks,
                    data_bitmap_blocks,
                    data_area_blocks,
                );
            },
        );
        // 创建根目录
        assert_eq!(efs.alloc_inode(), 0);
        let (root_inode_block_id, root_inode_offset) = efs.get_disk_inode_pos(0);
        get_block_cache(root_inode_block_id as usize, Arc::clone(&block_device))
            .lock()
            .modify(root_inode_offset, |disk_inode: &mut DiskInode| {
                disk_inode.initialize(DiskInodeType::Directory);
            });
        // 写回磁盘
        block_cache_sync_all();
        Arc::new(Mutex::new(efs))
    }

    // 从块设备中读取超级块，打开文件系统
    pub fn open(block_device: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        get_block_cache(0, Arc::clone(&block_device))
            .lock()
            .read(0, |super_block: &SuperBlock| {
                assert!(super_block.is_valid(), "Error loading EFS!");
                let inode_total_blocks =
                    super_block.inode_bitmap_blocks + super_block.inode_area_blocks;
                let efs = Self {
                    block_device,
                    inode_bitmap: Bitmap::new(1, super_block.inode_bitmap_blocks as usize),
                    data_bitmap: Bitmap::new(
                        (1 + inode_total_blocks) as usize,
                        super_block.data_bitmap_blocks as usize,
                    ),
                    inode_area_start_block: 1 + super_block.inode_bitmap_blocks,
                    data_area_start_block: 1 + inode_total_blocks + super_block.data_bitmap_blocks,
                };
                Arc::new(Mutex::new(efs))
            })
    }

    // 获取根目录的inode
    pub fn root_inode(efs: &Arc<Mutex<Self>>) -> Inode {
        let block_device = Arc::clone(&efs.lock().block_device);
        // acquire efs lock temporarily
        let (block_id, block_offset) = efs.lock().get_disk_inode_pos(0);
        // release efs lock
        Inode::new(block_id, block_offset, Arc::clone(efs), block_device)
    }

    // 拿到inode编号所在的块号和块内偏移
    pub fn get_disk_inode_pos(&self, inode_id: u32) -> (u32, usize) {
        let inode_size = core::mem::size_of::<DiskInode>();
        let inodes_per_block = (BLOCK_SZ / inode_size) as u32;
        let block_id = self.inode_area_start_block + inode_id / inodes_per_block;
        (
            block_id,
            (inode_id % inodes_per_block) as usize * inode_size,
        )
    }

    // 拿到数据块编号（在数据块位图中的编号）的块号
    pub fn get_data_block_id(&self, data_block_id: u32) -> u32 {
        self.data_area_start_block + data_block_id
    }

    // 分配一个inode
    pub fn alloc_inode(&mut self) -> u32 {
        self.inode_bitmap.alloc(&self.block_device).unwrap() as u32
    }

    // 分配一个数据块
    pub fn alloc_data(&mut self) -> u32 {
        self.data_bitmap.alloc(&self.block_device).unwrap() as u32 + self.data_area_start_block
    }

    // 释放一个数据块，将其缓冲区全部清零
    pub fn dealloc_data(&mut self, block_id: u32) {
        get_block_cache(block_id as usize, Arc::clone(&self.block_device))
            .lock()
            .modify(0, |data_block: &mut DataBlock| {
                data_block.iter_mut().for_each(|p| {
                    *p = 0;
                })
            });
        self.data_bitmap.dealloc(
            &self.block_device,
            (block_id - self.data_area_start_block) as usize,
        )
    }
}
