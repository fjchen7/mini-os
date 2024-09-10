// 开启no_std模式
#![no_std]
// 禁用main函数作为入口点
#![no_main]
// 开启panic_info_message特性，见[message](https://doc.rust-lang.org/std/panic/struct.PanicInfo.html#method.message)
#![feature(panic_info_message)]
// 开启alloc_error_handler特性
#![feature(alloc_error_handler)]
#![allow(unreachable_code)]
use core::arch::global_asm;
use task::TASK_MANAGER;

#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod logging;
mod mm;
mod sbi;
mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;

// 引入Rust内置的alloc库，用于动态内存分配
extern crate alloc;
#[macro_use]
extern crate bitflags;

// 载入汇编代码
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

// 不同的编译器，在编译时，会修改函数/变量的符号名，以做到解决命名冲突或增加类型安全等。
// 这叫做name mangling，不同的编译器有自身的策略。
//
// #[no_mangle]的作用：告诉编译器不修改函数的符号（name mangling）
// 我们要在汇编代码里调用Rust函数，因此要确保函数符号不被修改。否则，汇编代码将找不到该函数。
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    println_kernel!("Hello, world!");
    mm::init();
    println_kernel!("Init Memory Management");
    mm::remap_test();
    trap::init();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    TASK_MANAGER.run_first_task();
    panic!("Unreachable in rust_main!");
}

// 清零bss段（未初始化的全局变量）
fn clear_bss() {
    // 从链接器脚本（linker.ld）获取内存段的地址。通过引用外部的C函数来获取它们。
    extern "C" {
        fn sbss(); // .bss段的起始地址
        fn ebss(); // .bss段的结束地址
    }
    // 将.bss段清零
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) })
}
