//! 文件系统的磁盘布局

use crate::{block_cache::get_block_cache, block_dev::BlockDevice};

use super::BLOCK_SZ;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::{
    cmp::min,
    fmt::{Debug, Formatter, Result},
};

// 检查文件系统是否有效的魔数
const EFS_MAGIC: u32 = 0x3b800001;
// inode的文件名的最大长度
const NAME_LENGTH_LIMIT: usize = 27;

// 能用直接索引方式找到的块的数量
const INODE_DIRECT_COUNT: usize = 28;
// 能用一级间接索引方式找到的块的数量
const INODE_INDIRECT1_COUNT: usize = BLOCK_SZ / 4;
// 能用二级间接索引方式找到的块的数量
const INODE_INDIRECT2_COUNT: usize = INODE_INDIRECT1_COUNT * INODE_INDIRECT1_COUNT;
// 0..DIRECT_BOUND的块使用直接索引
const DIRECT_BOUND: usize = INODE_DIRECT_COUNT;
// DIRECT_BOUND..INDIRECT1_BOUND的块使用一级间接索引
const INDIRECT1_BOUND: usize = DIRECT_BOUND + INODE_INDIRECT1_COUNT;
#[allow(unused)]
// INDIRECT1_BOUND..INDIRECT2_BOUND的块使用二级间接索引
const INDIRECT2_BOUND: usize = INDIRECT1_BOUND + INODE_INDIRECT2_COUNT;

#[repr(C)]
// 文件系统的超级块
pub struct SuperBlock {
    // 用于检查文件系统是否有效的魔数
    magic: u32,
    // 总块数
    pub total_blocks: u32,
    // 下面表示inode位图、inode区域、数据位图、数据区域所占的块数
    pub inode_bitmap_blocks: u32,
    pub inode_area_blocks: u32,
    pub data_bitmap_blocks: u32,
    pub data_area_blocks: u32,
}

// Inode的类型
#[derive(PartialEq)]
pub enum DiskInodeType {
    File,
    Directory,
}

// 存放块索引的块对应的类型，每个元素是块编号（数据块或下一级的索引块）
type IndirectBlock = [u32; BLOCK_SZ / 4];
// 存放数据的块对应的类型
type DataBlock = [u8; BLOCK_SZ];

#[repr(C)]
// 文件/目录的inode结构
// 这里将DiskInode的结构设置为128字节，每个块恰好能存放4个inode
pub struct DiskInode {
    // 数据的字节大小
    pub size: u32,
    // inode的类型
    type_: DiskInodeType,
    // 有三个级别的索引，它们能同时使用。
    // 直接索引：直接指向块
    // 总共能容纳：INODE_DIRECT_COUNT * BLOCK_SZ ~= 14KB
    pub direct: [u32; INODE_DIRECT_COUNT],
    // 一级间接索引，指向一个包含多个块编号的块，每个编号是u32
    // 总共能容纳：(BLOCK_SZ / 4) * BLOCK_SZ ~= 64KB
    pub indirect1: u32,
    // 二级简介索引：指向一个包含多个一级间接索引块编号的块
    // 总共能容纳：(BLOCK_SZ / 4) * (BLOCK_SZ / 4) * BLOCK_SZ ~= 8MB
    pub indirect2: u32,
}

impl Debug for SuperBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("SuperBlock")
            .field("total_blocks", &self.total_blocks)
            .field("inode_bitmap_blocks", &self.inode_bitmap_blocks)
            .field("inode_area_blocks", &self.inode_area_blocks)
            .field("data_bitmap_blocks", &self.data_bitmap_blocks)
            .field("data_area_blocks", &self.data_area_blocks)
            .finish()
    }
}

impl SuperBlock {
    // 初始化
    pub fn initialize(
        &mut self,
        total_blocks: u32,
        inode_bitmap_blocks: u32,
        inode_area_blocks: u32,
        data_bitmap_blocks: u32,
        data_area_blocks: u32,
    ) {
        *self = Self {
            magic: EFS_MAGIC,
            total_blocks,
            inode_bitmap_blocks,
            inode_area_blocks,
            data_bitmap_blocks,
            data_area_blocks,
        }
    }

    // 通过魔数判断文件系统是否有效
    pub fn is_valid(&self) -> bool {
        self.magic == EFS_MAGIC
    }
}

impl DiskInode {
    // 初始化inode。一开始只使用直接索引，当数据块不够用时，再分配一级和二级间接索引
    pub fn initialize(&mut self, type_: DiskInodeType) {
        self.size = 0;
        self.direct.iter_mut().for_each(|v| *v = 0);
        self.indirect1 = 0;
        self.indirect2 = 0;
        self.type_ = type_;
    }

    pub fn is_dir(&self) -> bool {
        self.type_ == DiskInodeType::Directory
    }

    #[allow(unused)]
    pub fn is_file(&self) -> bool {
        self.type_ == DiskInodeType::File
    }

    // 返回存放数据所需的块数量（不包括inode的索引）
    pub fn data_blocks(&self) -> u32 {
        Self::_data_blocks(self.size)
    }

    fn _data_blocks(size: u32) -> u32 {
        size.div_ceil(BLOCK_SZ as u32)
    }

    // 返回存放数据及其inode的一级/二级间接索引所需的块数量
    pub fn total_blocks(size: u32) -> u32 {
        let data_blocks = Self::_data_blocks(size) as usize;
        let mut total = data_blocks;
        // 一级间接索引
        if data_blocks > DIRECT_BOUND {
            total += 1;
        }
        // 二级间接索引
        if data_blocks > INDIRECT1_BOUND {
            // 存放二级间接索引的块
            total += 1;
            // 向上取整
            total += (data_blocks - INDIRECT1_BOUND).div_ceil(INODE_INDIRECT1_COUNT);
        }
        total as u32
    }

    // 计算将数据大小扩展到new_size，还需要多少个块
    pub fn blocks_num_needed(&self, new_size: u32) -> u32 {
        assert!(new_size >= self.size);
        Self::total_blocks(new_size) - Self::total_blocks(self.size)
    }

    // 拿到第inner_id个的块编号。返回0表示没有分配
    pub fn get_block_id(&self, inner_id: u32, block_device: &Arc<dyn BlockDevice>) -> u32 {
        let inner_id = inner_id as usize;
        if inner_id < INODE_DIRECT_COUNT {
            self.direct[inner_id]
        } else if inner_id < INDIRECT1_BOUND {
            get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect_block: &IndirectBlock| {
                    indirect_block[inner_id - INODE_DIRECT_COUNT]
                })
        } else {
            let last = inner_id - INDIRECT1_BOUND;
            let indirect1 = get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect2: &IndirectBlock| {
                    indirect2[last / INODE_INDIRECT1_COUNT]
                });
            get_block_cache(indirect1 as usize, Arc::clone(block_device))
                .lock()
                .read(0, |indirect1: &IndirectBlock| {
                    indirect1[last % INODE_INDIRECT1_COUNT]
                })
        }
    }

    // 将数据大小扩容到new_size。
    // new_blocks是扩容要用的块编号，需要提前算出来，只能多不能少。
    pub fn increase_size(
        &mut self,
        new_size: u32,
        new_blocks: Vec<u32>,
        block_device: &Arc<dyn BlockDevice>,
    ) {
        let mut current_blocks = self.data_blocks();
        self.size = new_size;
        let mut total_blocks = self.data_blocks();
        let mut new_blocks = new_blocks.into_iter();
        // 分配给直接索引，并更新直接索引数组
        while current_blocks < min(total_blocks, INODE_DIRECT_COUNT as u32) {
            self.direct[current_blocks as usize] = new_blocks.next().unwrap();
            current_blocks += 1;
        }
        // 如果不够，分配一级索引
        if total_blocks > INODE_DIRECT_COUNT as u32 {
            if current_blocks == INODE_DIRECT_COUNT as u32 {
                self.indirect1 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_DIRECT_COUNT as u32;
            total_blocks -= INODE_DIRECT_COUNT as u32;
        } else {
            return;
        }
        // 更新一级索引块
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < min(total_blocks, INODE_INDIRECT1_COUNT as u32) {
                    indirect1[current_blocks as usize] = new_blocks.next().unwrap();
                    current_blocks += 1;
                }
            });
        // 如果还不够，分配二级索引
        if total_blocks > INODE_INDIRECT1_COUNT as u32 {
            if current_blocks == INODE_INDIRECT1_COUNT as u32 {
                self.indirect2 = new_blocks.next().unwrap();
            }
            current_blocks -= INODE_INDIRECT1_COUNT as u32;
            total_blocks -= INODE_INDIRECT1_COUNT as u32;
        } else {
            return;
        }
        // 更新二级索引块
        // 对于某个要分配的块，a0/b0表示：
        // - a0：二级索引中，对应项的偏移
        // - a0：一级索引（由二级索引找过来的）中，对应项的偏移
        let mut a0 = current_blocks as usize / INODE_INDIRECT1_COUNT;
        let mut b0 = current_blocks as usize % INODE_INDIRECT1_COUNT;
        let a1 = total_blocks as usize / INODE_INDIRECT1_COUNT;
        let b1 = total_blocks as usize % INODE_INDIRECT1_COUNT;
        // alloc low-level indirect1
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                while (a0 < a1) || (a0 == a1 && b0 < b1) {
                    if b0 == 0 {
                        indirect2[a0] = new_blocks.next().unwrap();
                    }
                    // fill current
                    get_block_cache(indirect2[a0] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            indirect1[b0] = new_blocks.next().unwrap();
                        });
                    // move to next
                    b0 += 1;
                    if b0 == INODE_INDIRECT1_COUNT {
                        b0 = 0;
                        a0 += 1;
                    }
                }
            });
    }

    // 释放inode所使用的块（包括存放数据和间接索引的块）。只是释放，并不清空缓冲区或磁盘上的数据。
    // 返回释放的块编号
    pub fn clear_size(&mut self, block_device: &Arc<dyn BlockDevice>) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let mut data_blocks = self.data_blocks() as usize;
        self.size = 0;
        let mut current_blocks = 0usize;
        // direct
        while current_blocks < min(data_blocks, INODE_DIRECT_COUNT) {
            v.push(self.direct[current_blocks]);
            self.direct[current_blocks] = 0;
            current_blocks += 1;
        }
        // indirect1 block
        if data_blocks > INODE_DIRECT_COUNT {
            v.push(self.indirect1);
            data_blocks -= INODE_DIRECT_COUNT;
            current_blocks = 0;
        } else {
            return v;
        }
        // indirect1
        get_block_cache(self.indirect1 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect1: &mut IndirectBlock| {
                while current_blocks < min(data_blocks, INODE_INDIRECT1_COUNT) {
                    v.push(indirect1[current_blocks]);
                    //indirect1[current_blocks] = 0;
                    current_blocks += 1;
                }
            });
        self.indirect1 = 0;
        // indirect2 block
        if data_blocks > INODE_INDIRECT1_COUNT {
            v.push(self.indirect2);
            data_blocks -= INODE_INDIRECT1_COUNT;
        } else {
            return v;
        }
        // indirect2
        assert!(data_blocks <= INODE_INDIRECT2_COUNT);
        let a1 = data_blocks / INODE_INDIRECT1_COUNT;
        let b1 = data_blocks % INODE_INDIRECT1_COUNT;
        get_block_cache(self.indirect2 as usize, Arc::clone(block_device))
            .lock()
            .modify(0, |indirect2: &mut IndirectBlock| {
                // full indirect1 blocks
                for entry in indirect2.iter_mut().take(a1) {
                    v.push(*entry);
                    get_block_cache(*entry as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter() {
                                v.push(*entry);
                            }
                        });
                }
                // last indirect1 block
                if b1 > 0 {
                    v.push(indirect2[a1]);
                    get_block_cache(indirect2[a1] as usize, Arc::clone(block_device))
                        .lock()
                        .modify(0, |indirect1: &mut IndirectBlock| {
                            for entry in indirect1.iter().take(b1) {
                                v.push(*entry);
                            }
                        });
                    //indirect2[a1] = 0;
                }
            });
        self.indirect2 = 0;
        v
    }

    // 从inode中读取数据到buf中，返回读取的字节数
    pub fn read_at(
        &self,
        offset: usize,
        buf: &mut [u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = min(offset + buf.len(), self.size as usize);
        if start >= end {
            return 0;
        }
        let mut start_block = start / BLOCK_SZ;
        let mut read_size = 0usize;
        loop {
            // 当前块的结束位置（结束位置的字节偏移量）
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = min(end_current_block, end);
            // 读取的字节数
            let block_read_size = end_current_block - start;
            let dst = &mut buf[read_size..read_size + block_read_size];
            let block_id = self.get_block_id(start_block as u32, block_device) as usize;
            get_block_cache(block_id, Arc::clone(block_device))
                .lock()
                .read(0, |data_block: &DataBlock| {
                    let src = &data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_read_size];
                    dst.copy_from_slice(src);
                });
            read_size += block_read_size;
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        read_size
    }

    // 从buf中写入数据到inode中，返回写入的字节数
    pub fn write_at(
        &mut self,
        offset: usize,
        buf: &[u8],
        block_device: &Arc<dyn BlockDevice>,
    ) -> usize {
        let mut start = offset;
        let end = min(offset + buf.len(), self.size as usize);
        assert!(start <= end);
        let mut start_block = start / BLOCK_SZ;
        let mut write_size = 0usize;
        loop {
            // 当前块的结束位置的字节偏移量
            let mut end_current_block = (start / BLOCK_SZ + 1) * BLOCK_SZ;
            end_current_block = min(end_current_block, end);
            // 写入的字节数
            let block_write_size = end_current_block - start;
            let block_id = self.get_block_id(start_block as u32, block_device) as usize;
            get_block_cache(block_id, Arc::clone(block_device))
                .lock()
                .modify(0, |data_block: &mut DataBlock| {
                    let src = &buf[write_size..write_size + block_write_size];
                    let dst =
                        &mut data_block[start % BLOCK_SZ..start % BLOCK_SZ + block_write_size];
                    dst.copy_from_slice(src);
                });
            write_size += block_write_size;
            if end_current_block == end {
                break;
            }
            start_block += 1;
            start = end_current_block;
        }
        write_size
    }
}

#[repr(C)]
// 目录下的一个目录项
// 类型为目录的DiskInode中，它的数据块存放的是DirEntry
pub struct DirEntry {
    // 多1个字节，用来存放\0
    name: [u8; NAME_LENGTH_LIMIT + 1],
    inode_number: u32,
}
// 目录项的大小（ 27 + 1 + 4 = 32）
pub const DIRENT_SZ: usize = 32;

impl DirEntry {
    // 创建一个空的目录项
    pub fn empty() -> Self {
        Self {
            name: [0u8; NAME_LENGTH_LIMIT + 1],
            inode_number: 0,
        }
    }
    // 创建一个新的目录项
    pub fn new(name: &str, inode_number: u32) -> Self {
        let mut bytes = [0u8; NAME_LENGTH_LIMIT + 1];
        bytes[..name.len()].copy_from_slice(name.as_bytes());
        Self {
            name: bytes,
            inode_number,
        }
    }

    // 将自身序列化为字节数组
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self as *const _ as usize as *const u8, DIRENT_SZ) }
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self as *mut _ as usize as *mut u8, DIRENT_SZ) }
    }

    pub fn name(&self) -> &str {
        let len = (0usize..).find(|i| self.name[*i] == 0).unwrap();
        core::str::from_utf8(&self.name[..len]).unwrap()
    }

    pub fn inode_number(&self) -> u32 {
        self.inode_number
    }
}
