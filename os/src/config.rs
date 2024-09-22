//! 一些配置

// 用户栈和内核栈的大小（KB）
pub const USER_STACK_SIZE: usize = 4096;
pub const KERNEL_STACK_SIZE: usize = 4096 * 2;
pub const KERNEL_HEAP_SIZE: usize = 0x30_0000;
// 页面大小为4KB
pub const PAGE_SIZE: usize = 4096;
// 需要12位才能表示页面的任意位置。这是页内偏移（Page Offset）的位长。
pub const PAGE_SIZE_BITS: usize = 12;

// 空间地址的高256GB存放（按高位到低位）：
// - 跳板（Trampoline）：存放__alltraps和__restore代码，用于进入/退出Trap
// - TrapContext：保存Trap的上下文
pub const TRAMPOLINE: usize = usize::MAX - PAGE_SIZE + 1;
pub const TRAP_CONTEXT: usize = TRAMPOLINE - PAGE_SIZE;

// 物理内存的结束地址
// 在linker.ld中，我们将将内核数据的结束地址（ekernel）定为0x80_000_000
// 因此我们的物理内存大小为8MB
pub const MEMORY_END: usize = 0x88_000_000;

// CPU的时钟频率（Hz），即每秒CPU经过的时钟周期数。
// 这也是计数器寄存器mtime每秒会增加的数字。
pub const CLOCK_FREQ: usize = 12500000;

// MMIO可将设备的寄存器映射到内存中，以便CPU就能通过读写内存来控制该设备。
pub const MMIO: &[(usize, usize)] = &[
    // (0x0010_0000, 0x00_2000), // VIRT_TEST/RTC  in virt machine
    // Qemu模拟器中，MMIO的地址从0x1000_0000开始，大小为4KB
    (0x1000_1000, 0x00_1000), // Virtio Block in virt machine
];
