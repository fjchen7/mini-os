use core::cmp::min;

use alloc::{
    collections::{btree_map::BTreeMap, btree_set::BTreeSet},
    sync::Arc,
    vec::Vec,
};
use easy_fs::Inode;

use crate::config::PAGE_SIZE;

use super::{
    address::VirtPageNum, frame_alloc, FrameTracker, MapPermission, PhysPageNum, VirtAddr,
};

// 描述文件到内存的映射关系（mmap）
// 每个文件可以有多个映射区域。它们分别映射到不同的虚拟内存区域。
// 每个虚拟内存区域连续，但不同区域之间可以不连续。每个区域有独立的权限。
// 当前只允许将单个进程的文件映射到多个区域，不允许多个进程映射到同一个文件。
pub struct FileMapping {
    // 被映射的文件。只能是常规文件，所以使用Inode。
    pub file: Arc<Inode>,
    // 映射到的虚拟内存区域。
    // 注意，当前不允许该虚拟地址区域重叠
    ranges: Vec<MapRange>,
    // 实际映射到的物理页号
    frames: Vec<FrameTracker>,
    // 需要写回磁盘的虚拟页号（脏位）
    dirty_parts: BTreeSet<usize>,
    // 文件内的偏移，到物理页号的映射
    map: BTreeMap<usize, PhysPageNum>,
}

#[derive(Clone)]
// 表示文件中的[offset, offset+len)区间，映射到虚拟地址[start, start+len)区间]
pub struct MapRange {
    // 文件中的偏移量（起始位置）
    offset: usize,
    // 长度（字节）
    len: usize,
    // 该偏移量映射到的虚拟地址。
    // 注意，虚拟内存是按页分配的。如果start在页中间，那该页的前半部分就用不到。
    start: VirtAddr,
    pub perm: MapPermission,
}

impl FileMapping {
    pub fn new_empty(file: Arc<Inode>) -> Self {
        Self {
            file,
            ranges: Vec::new(),
            frames: Vec::new(),
            dirty_parts: BTreeSet::new(),
            map: BTreeMap::new(),
        }
    }

    pub fn push(&mut self, start: VirtAddr, len: usize, offset: usize, perm: MapPermission) {
        self.ranges.push(MapRange {
            start,
            len,
            offset,
            perm,
        });
    }

    pub fn contains(&self, va: VirtAddr) -> bool {
        self.ranges.iter().any(|r| r.contains(va))
    }

    // 为给定的虚拟地址，映射到物理页号
    // 返回值：物理页号、映射区域、是否共享（如果先前已经映射过，那就是共享的）
    // 如果先前已经映射过，那么不会再次分配物理页号
    pub fn map(&mut self, va: VirtAddr) -> Option<(PhysPageNum, MapRange, bool)> {
        let vpn = va.floor();
        for range in &self.ranges {
            if !range.contains(va) {
                continue;
            }
            // 计算该虚拟页号，在文件中的偏移量
            let offset = range.file_offset(vpn);
            // 查找该虚拟页号，是否已经映射到物理页号
            let (ppn, shared) = match self.map.get(&offset) {
                // 如果已经映射到物理页号，直接返回
                Some(&ppn) => (ppn, true),
                None => {
                    // 否则分配一个物理页
                    let frame = frame_alloc().unwrap();
                    let ppn = frame.ppn;
                    self.frames.push(frame);
                    self.map.insert(offset, ppn);
                    (ppn, false)
                }
            };
            if range.perm.contains(MapPermission::W) {
                self.dirty_parts.insert(offset);
            }
            return Some((ppn, range.clone(), shared));
        }
        None
    }

    pub fn sync(&self) {
        let file_size = self.file.size() as usize;
        for &offset in self.dirty_parts.iter() {
            let ppn = self.map.get(&offset).unwrap();
            if offset < file_size {
                // WARNING: this can still cause garbage written
                //  to file when sharing physical page
                let va_len = self
                    .ranges
                    .iter()
                    .map(|r| {
                        if r.offset <= offset && offset < r.offset + r.len {
                            min(PAGE_SIZE, r.offset + r.len - offset)
                        } else {
                            0
                        }
                    })
                    .max()
                    .unwrap();
                let write_len = va_len.min(file_size - offset);

                self.file
                    .write_at(offset, &ppn.get_bytes_array()[..write_len]);
            }
        }
    }
}

impl MapRange {
    // 该虚拟内存区间，是否包含给定的虚拟地址
    fn contains(&self, va: VirtAddr) -> bool {
        let start: usize = self.start.into();
        let va: usize = va.into();
        va >= start && va < start + self.len
    }

    // 计算给定虚拟页号在文件中的偏移量
    pub fn file_offset(&self, vpn: VirtPageNum) -> usize {
        let start: usize = self.start.into();
        let va: VirtAddr = vpn.into();
        let va: usize = va.into();
        self.offset + (va - start)
    }
}

// 选一段没人用的地址空间作为mmap的基址
pub const MMAP_AREA_BASE: usize = 0x0000_0001_0000_0000;

pub struct VirtualAddressAllocator {
    cur_va: VirtAddr,
}

impl Default for VirtualAddressAllocator {
    fn default() -> Self {
        Self::new(MMAP_AREA_BASE)
    }
}

impl VirtualAddressAllocator {
    pub fn new(base: usize) -> Self {
        Self {
            cur_va: base.into(),
        }
    }

    // 分配一段虚拟地址区域
    pub fn alloc(&mut self, len: usize) -> VirtAddr {
        let start = self.cur_va;
        let end: VirtAddr = (self.cur_va.0 + len).into();
        self.cur_va = end.ceil().into();
        start
    }
}
