//! 管理页帧（frame），即物理页

use crate::{config::MEMORY_END, mm::address::PhysAddr, sync::UPSafeCell};
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::*;

use super::address::PhysPageNum;

trait FrameAllocator {
    fn new() -> Self;
    // 分配物理页帧
    fn alloc(&mut self) -> Option<PhysPageNum>;
    // 回收物理页帧
    fn dealloc(&mut self, ppn: PhysPageNum);
}

// 栈式物理页帧分配器
pub struct StackFrameAllocator {
    current: usize, // 空闲内存的起始物理页号
    end: usize,     // 空闲内存的结束物理页号
    recycled: Vec<usize>,
}

impl StackFrameAllocator {
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
        // 优先使用回收的物理页帧
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            let allocated = self.current;
            self.current += 1;
            Some(allocated.into())
        }
    }

    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // 合法性检查
        // - 该页面是被分配过
        // - 该页面没有被回收
        if ppn >= self.current || self.recycled.iter().any(|&v| v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn);
        }
        // 回收物理页帧
        self.recycled.push(ppn);
    }
}

// 该类型用于管理物理页帧的生命周期
pub struct FrameTracker {
    pub ppn: PhysPageNum,
}

impl FrameTracker {
    pub fn new(ppn: PhysPageNum) -> Self {
        // 清理物理页帧中的内容
        ppn.get_bytes_array().iter_mut().for_each(|i| *i = 0);
        Self { ppn }
    }
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker:PPN={:#x}", self.ppn.0))
    }
}

// FrameTracker生命周期结束时，要确保物理页帧被回收
impl Drop for FrameTracker {
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

lazy_static! {
    // 全局的物理页帧分配器
    pub static ref FRAME_ALLOCATOR: UPSafeCell<StackFrameAllocator> =
        unsafe { UPSafeCell::new(StackFrameAllocator::new()) };
}

// 初始化全局物理页帧分配器
pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel(); // linker.ld中定义的内核数据的物理内存结束地址
    }
    // 可供分配的物理页号范围：[ekernel向上取整转化的物理页号, MEMORY_END向下取整转化的物理页号)
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

// 分配一个物理页帧
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

// 回收物理页帧
fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}
