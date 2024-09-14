#![no_std]
#![feature(panic_info_message)]
#![feature(linkage)] // 支持弱链接的标记
#![feature(alloc_error_handler)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

#[macro_use]
extern crate bitflags;

use buddy_system_allocator::LockedHeap;
const USER_HEAP_SIZE: usize = 16384;
static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

// 指定全局内存分配器，以使用需要堆分配的数据结构，如String、Vec等。
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[no_mangle]
// 链接时用的符号，分为强链接和弱链接：
// - 链接时，弱链接可以未定义，不会报错。但强链接必须定义。
// - 链接时，如果存在同名的强链接和弱链接符号，选择强链接。
// - 运行时，目标文件提供弱链接的定义，并覆盖默认的（如果有的话）。如果有多个定义，链接器会选择其中一个（根据其策略）。
//
// 这里将入口函数main标记为弱链接，且提供了默认实现。
// bin目录下的每个应用程序，都会提供自己的main函数。在运行时，会覆盖这里的默认main实现。
#[linkage = "weak"]
fn main() -> i32 {
    panic!("Cannot find main!");
}

// 定义user库的入口
#[no_mangle]
// 将该函数编译后的汇编代码，放入内存段.text.entry，表示程序的入口点（见文件linker.ld）。
#[link_section = ".text.entry"]
pub extern "C" fn _start() -> ! {
    unsafe {
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
    // 下面要进入的main方法，由应用程序各自实现。链接生成ELF时，会替换此处的默认main实现。
    let exist_code = main();
    exit(exist_code);
}

use syscall::*;

bitflags! {
    pub struct OpenFlags: u32 {
        const RDONLY = 0;       // 只读
        const WRONLY = 1 << 0;  // 只写
        const RDWR = 1 << 1;    // 读写
        const CREATE = 1 << 9;  // 创建。如果文件存在，则截断文件
        const TRUNC = 1 << 10;  // 截断，即删除文件中原有的内容
    }
}

pub fn open(path: &str, flags: OpenFlags) -> isize {
    sys_open(path, flags.bits)
}

pub fn close(fd: usize) -> isize {
    sys_close(fd)
}

pub fn read(fd: usize, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}

pub fn write(fd: usize, buf: &[u8]) -> isize {
    sys_write(fd, buf)
}
pub fn exit(exit_code: i32) -> ! {
    sys_exit(exit_code)
}
pub fn yield_() -> isize {
    sys_yield()
}
pub fn get_time() -> isize {
    sys_get_time()
}

pub fn sleep(period_ms: usize) {
    let start = sys_get_time();
    while sys_get_time() < start + period_ms as isize {
        sys_yield();
    }
}

pub fn sbrk(size: i32) -> isize {
    sys_sbrk(size)
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn fork() -> isize {
    sys_fork()
}

pub fn exec(path: &str) -> isize {
    sys_exec(path)
}

// 等待任意一个子进程结束
pub fn wait(exit_code: &mut i32) -> isize {
    blocking_waitpid(-1, exit_code)
}

// 等待指定pid的子进程结结束
pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    blocking_waitpid(pid as isize, exit_code)
}

// 等待指定pid的子进程结束，并回收其资源。pid为-1时，表示等待任意子进程。
fn blocking_waitpid(pid: isize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid, exit_code as *mut _) {
            // 如果子进程都未结束，则让出CPU
            -2 => {
                sys_yield();
            }
            // 返回子进程的PID（正常结束）或-1（子进程不存在）
            exit_pid => return exit_pid,
        }
    }
}
