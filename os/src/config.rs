//! 一些配置

// 用户栈和内核栈的大小（KB）
pub const USER_STACK_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const MAX_APP_NUM: usize = 10;
pub const APP_BASE_ADDRESS: usize = 0x80400000;
pub const APP_SIZE_LIMIT: usize = 0x20000;

// CPU的时钟频率（Hz），即每秒CPU经过的时钟周期数。
// 这也是计数器寄存器mtime每秒会增加的数字。
pub const CLOCK_FREQ: usize = 12500000;
