//! 页表的数据结构表示，以及多级页表的实现。

use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;
use core::cmp::min;

use super::{
    address::{PhysPageNum, StepByOne as _, VirtAddr, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
};

// bitflags!能生成表示标志位的结构体
bitflags! {
    // 页表项的标志位
    pub struct PTEFlags: u8 {
        const V = 1 << 0;  // 页表是否合法
        const R = 1 << 1;  // 可读
        const W = 1 << 2;  // 可写
        const X = 1 << 3;  // 可执行
        const U = 1 << 4;  // 用户态（CPU处于U特权级时）可访问
        const G = 1 << 5;
        const A = 1 << 6;  // 已被访问
        const D = 1 << 7;  // 已被修改
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
// 页表项（Page Table Entry）是页表中的一个元素，用于存储虚拟页号到物理页号的映射关系。
// 页表项为64位，结构如下：
//   - 高10位：保留位
//   - 接下来44位：物理页号（PPN）
//   - 接下来2位：保留位
//   - 低8位：标志位
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }

    pub fn ppn(&self) -> PhysPageNum {
        // 取bits[53:10]作为物理页号，共44位
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }

    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }

    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }

    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }

    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

// 多级页表。每个应用程序都有自己的页表。
pub struct PageTable {
    // 根页表的物理页号
    root_ppn: PhysPageNum,
    // 保存页表所在的物理页帧
    frames: Vec<FrameTracker>,
}

// 为了简化实现，这里假设创建和映射页表时不会发生内存分配失败。
impl PageTable {
    pub fn new() -> Self {
        // 分配一个物理页，作为根页表
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    // 模拟MMU从satp寄存器中创建页表
    // 这里的变量表示CSR寄存器satp的值，其最低位的44位表示根页表的物理页号
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    // 找到虚拟页号对应的页表项，返回其拷贝。
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    // 找到虚拟页号对应的页表项，如果不存在则创建。
    // 但返回的页表项不一定合法，需要调用者进一步判断。
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&'static mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            // 找到页表中对应的页表项
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    // 找到虚拟页号对应的页表项。如果不存在，则返回None。
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&'static mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    #[allow(unused)]
    // 将虚拟页号映射到物理页号
    // 这里采用恒等映射，即虚拟页号等于物理页号
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        // 如果找到的页表项是合法的，则表示之前已经映射过了，报错。
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    #[allow(unused)]
    // 取消虚拟页号的映射
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        // 如果找到的页表项是非法的，则表示之前没有映射过，报错。
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }

    // 构造CSR寄存器satp的值，使得分页模式为SV39。satp用于控制MMU的行为。
    // CSR寄存器satp的格式：MODE (4 bits) | ASID (16 bits) | PPN (44 bits)
    // - MODE：0不开启分页机制，8开启SV39分页机制
    // - ASCI：地址空间的标识符
    // - PPN：根页表的物理页号
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

// 读出某个地址空间中的一段缓冲区中的数据
// token表示地址空间，ptr和len分别表示该地址空间中要读出的区域的起始地址和长度
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let end_va = min(VirtAddr::from(vpn), VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}
