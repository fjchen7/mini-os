//! 一些配置

// 用户栈和内核栈的大小（KB）
pub const USER_STACK_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
// 页面大小为4KB
pub const PAGE_SIZE: usize = 4096;
// 需要12位才能表示页面的任意位置。这是页内偏移（Page Offset）的位长。
pub const PAGE_SIZE_BITS: usize = 12;
pub const MAX_APP_NUM: usize = 10;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

// 物理内存的结束地址
// 在linker.ld中，我们将将内核数据的结束地址（ekernel）定为0x80_000_000
// 因此我们的物理内存大小为8MB
pub const MEMORY_END: usize = 0x88_000_000;

// CPU的时钟频率（Hz），即每秒CPU经过的时钟周期数。
// 这也是计数器寄存器mtime每秒会增加的数字。
pub const CLOCK_FREQ: usize = 12500000;
