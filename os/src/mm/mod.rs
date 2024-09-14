//! 基于RISC-V的SV39分页机制的内存管理实现。
//! 分配器、页表、映射方式和内存集合表示，都在这里实现。
//!
//! 每个任务或进程都有一个内存集合，用于管理其虚拟内存。

mod address;
mod file_mapping;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, StepByOne, VirtAddr};
pub use file_mapping::{FileMapping, VirtualAddressAllocator};
pub use frame_allocator::{frame_alloc, frame_dealloc, FrameTracker};
pub use memory_set::{kernel_token, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{
    translated_byte_buffer, translated_refmut, translated_str, PageTable, UserBuffer,
};

// 初始化内存管理模块
pub fn init() {
    // 初始化堆分配器
    heap_allocator::init_heap();
    heap_allocator::heap_test();
    // 初始化物理页帧分配器
    frame_allocator::init_frame_allocator();
    // 初始化内核空间
    KERNEL_SPACE.exclusive_access().activate();
    memory_set::remap_test();
}
