//! 页表的数据结构表示，以及多级页表的实现。

use bitflags::*;

use super::address::PhysPageNum;

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
