//! 定义空间地址相关的类型：虚拟地址、物理地址、虚拟页号、物理页号。
//!
//! 提供相关的转换函数，方便进行地址转换。

use crate::config::{PAGE_SIZE, PAGE_SIZE_BITS};
use core::fmt::{self, Debug, Formatter};

use super::page_table::PageTableEntry;

// RISC-V的SV39分页机制中，虚拟地址为39位，物理地址为56位。
const VA_WIDTH_SV39: usize = 39;
const PA_WIDTH_SV39: usize = 56;
const VPN_WIDTH_SV39: usize = VA_WIDTH_SV39 - PAGE_SIZE_BITS;
const PPN_WIDTH_SV39: usize = PA_WIDTH_SV39 - PAGE_SIZE_BITS;

// 虚拟地址（VA，virtual address），为39位
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

// 物理地址（PA，physical address），为56位
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

// 虚拟页号（VPN，virtual page number），为39-12=27位
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);

// 物理页号（PPN，physical page number），为56-12=44位
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VA:{:#x}", self.0))
    }
}
impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VPN:{:#x}", self.0))
    }
}
impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PA:{:#x}", self.0))
    }
}
impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PPN:{:#x}", self.0))
    }
}

impl From<usize> for PhysAddr {
    fn from(v: usize) -> Self {
        // 只取v的低56（PA_WIDTH_SV39）位作为物理地址
        Self(v & ((1 << PA_WIDTH_SV39) - 1))
    }
}
impl From<usize> for PhysPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << PPN_WIDTH_SV39) - 1))
    }
}
impl From<usize> for VirtAddr {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VA_WIDTH_SV39) - 1))
    }
}
impl From<usize> for VirtPageNum {
    fn from(v: usize) -> Self {
        Self(v & ((1 << VPN_WIDTH_SV39) - 1))
    }
}

impl From<PhysAddr> for usize {
    fn from(v: PhysAddr) -> Self {
        v.0
    }
}
impl From<PhysPageNum> for usize {
    fn from(v: PhysPageNum) -> Self {
        v.0
    }
}
impl From<VirtAddr> for usize {
    fn from(v: VirtAddr) -> Self {
        // RISC-V的虚拟地址跟位宽一样，有64位。
        // 但开启SV39后，要求没用到的高25位与第38位相同，否则MMU会将认为它是不合法的虚拟地址（触发Page Fault）。

        // 这个if判断第38位是否为1
        if v.0 >= (1 << (VA_WIDTH_SV39 - 1)) {
            // 如果第38位是1，就把高25位也都设置为1
            v.0 | (!((1 << VA_WIDTH_SV39) - 1))
        } else {
            v.0
        }
    }
}
impl From<VirtPageNum> for usize {
    fn from(v: VirtPageNum) -> Self {
        v.0
    }
}

impl VirtAddr {
    // 返回虚拟页号，向下取值
    pub fn floor(&self) -> VirtPageNum {
        VirtPageNum(self.0 / PAGE_SIZE)
    }
    // 返回虚拟页号，向上取值
    pub fn ceil(&self) -> VirtPageNum {
        if self.0 == 0 {
            VirtPageNum(0)
        } else {
            VirtPageNum((self.0 + (PAGE_SIZE - 1)) / PAGE_SIZE)
        }
    }
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }
}
impl From<VirtAddr> for VirtPageNum {
    fn from(v: VirtAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}
impl From<VirtPageNum> for VirtAddr {
    fn from(v: VirtPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}

impl PhysAddr {
    // 返回物理页号，向下取值
    pub fn floor(&self) -> PhysPageNum {
        PhysPageNum(self.0 / PAGE_SIZE)
    }
    // 返回物理页号，向上取值
    pub fn ceil(&self) -> PhysPageNum {
        if self.0 == 0 {
            PhysPageNum(0)
        } else {
            PhysPageNum((self.0 - 1 + PAGE_SIZE) / PAGE_SIZE)
        }
    }
    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
    pub fn aligned(&self) -> bool {
        self.page_offset() == 0
    }

    pub fn get_mut<T>(&self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(v: PhysAddr) -> Self {
        assert_eq!(v.page_offset(), 0);
        v.floor()
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(v: PhysPageNum) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}

impl VirtPageNum {
    // 取出虚拟页号在页表中的索引
    // 虚拟页号共27位，按字节低位到高位，为：一级（根）索引（9位） | 二级索引（9位） | 三级索引（9位）
    // 该索引表示，该虚拟页号在三级页表中的位置
    pub fn indexes(&self) -> [usize; 3] {
        let mut vpn = self.0;
        let mut idx = [0usize; 3];
        for i in (0..3).rev() {
            // 取出低9位。最先取出的是一级（（根页表）索引
            idx[i] = vpn & 0b1_1111_1111;
            vpn >>= 9;
        }
        idx
    }
}

impl PhysPageNum {
    // 将该物理页存的4KB数据，转换成页表项数组，共4KB/8B=512个
    // 只有页表才会用到这个函数
    pub fn get_pte_array(&self) -> &'static mut [PageTableEntry] {
        // 得到该物理页的初始物理地址
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut PageTableEntry, 512) }
    }

    // 将该物理页所存的4KB数据，以字节形式读出
    pub fn get_bytes_array(&self) -> &'static mut [u8] {
        let pa: PhysAddr = (*self).into();
        unsafe { core::slice::from_raw_parts_mut(pa.0 as *mut u8, 4096) }
    }

    // 将该物理页所存的4KB数据，以T类型读出
    pub fn get_mut<T>(&self) -> &'static mut T {
        let pa: PhysAddr = (*self).into();
        unsafe { (pa.0 as *mut T).as_mut().unwrap() }
    }
}

pub trait StepByOne {
    fn step(&mut self);
}

impl StepByOne for VirtPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

impl StepByOne for PhysPageNum {
    fn step(&mut self) {
        self.0 += 1;
    }
}

#[derive(Copy, Clone)]
// 简单的Range结构体，范围为[l, r)
pub struct SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    l: T,
    r: T,
}

impl<T> SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "start {:?} > end {:?}!", start, end);
        Self { l: start, r: end }
    }

    pub fn get_start(&self) -> T {
        self.l
    }

    pub fn get_end(&self) -> T {
        self.r
    }
}

impl<T> IntoIterator for SimpleRange<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    type IntoIter = SimpleRangeIterator<T>;
    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIterator::new(self.l, self.r)
    }
}

pub struct SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    current: T,
    end: T,
}

impl<T> SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    pub fn new(l: T, r: T) -> Self {
        Self { current: l, end: r }
    }
}

impl<T> Iterator for SimpleRangeIterator<T>
where
    T: StepByOne + Copy + PartialEq + PartialOrd + Debug,
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let t = self.current;
            self.current.step();
            Some(t)
        }
    }
}

pub type VPNRange = SimpleRange<VirtPageNum>;
