// 开启no_std模式
#![no_std]
// 禁用main函数作为入口点
#![no_main]
// 开启panic_info_message特性，见[message](https://doc.rust-lang.org/std/panic/struct.PanicInfo.html#method.message)
#![feature(panic_info_message)]
// 开启alloc_error_handler特性
#![feature(alloc_error_handler)]
#![allow(unreachable_code)]

#[macro_use]
mod console;
mod board;
mod config;
mod drivers;
pub mod fs;
mod lang_items;
mod logging;
mod mm;
mod sbi;
mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;

// 引入内置的alloc库，用于动态内存分配
extern crate alloc;
#[macro_use]
extern crate bitflags;

// 加载汇编代码
use core::arch::global_asm;

use drivers::{CharDevice as _, DEV_NON_BLOCKING_ACCESS, GPU_DEVICE, UART};
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

// 编译器在编译时，可能修改函数/变量的符号名，来解决命名冲突、保证类型安全或做到其他优化。
// 这叫做name mangling，不同的编译器有不同的策略。
//
// 我们要在汇编代码里调用rust_main方法。它是通过该函数的符号名来找到该方法的。
// #[no_mangle]的作用是，告诉编译器不要修改函数的符号名。这样汇编代码才能找到该函数。
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    mm::init();
    UART.init();
    println!("KERN: init gpu");
    let _gpu = GPU_DEVICE.clone();
    trap::init();
    trap::enable_timer_interrupt();
    task::add_initproc();
    timer::set_next_trigger();
    board::device_init();
    fs::list_apps();
    *DEV_NON_BLOCKING_ACCESS.exclusive_access() = true;
    println_kernel!("Hello, world!");
    task::run_tasks();
    unreachable!("Never reach end of rust_main");
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
