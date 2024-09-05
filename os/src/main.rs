// 开启no_std模式
#![no_std]
// 禁用main函数作为入口点
#![no_main]
// 开启panic_info_message特性，见[message](https://doc.rust-lang.org/std/panic/struct.PanicInfo.html#method.message)
#![feature(panic_info_message)]

use core::arch::global_asm;
use log::*;
use task::TASK_MANAGER;

#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod logging;
mod sbi;
mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;

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
    // 内核初始化
    clear_bss();
    log_memory();
    println!("Hello, world!");
    trap::init();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_trigger(); // 设置第一个时钟中断
    TASK_MANAGER.run_first_task();
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

fn log_memory() {
    logging::init();
    extern "C" {
        // .text段的起始和结束地址
        fn stext();
        fn etext();
        // .rodata段的起始和结束地址
        fn srodata();
        fn erodata();
        // .data段的起始和结束地址
        fn sdata();
        fn edata();
        // .bss段的起始和结束地址
        fn sbss();
        fn ebss();
        fn boot_stack_lower_bound(); // 栈底
        fn boot_stack_top(); // 栈顶
    }
    warn!(
        "[kernel] boot_stack top=bottom={:#x}, lower_bound={:#x}",
        boot_stack_top as usize, boot_stack_lower_bound as usize
    );
    error!("[kernel] .bss [{:#x}, {:#x})", sbss as usize, ebss as usize);
    info!(
        "[kernel] .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );
    debug!(
        "[kernel] .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );
    trace!(
        "[kernel] .text [{:#x}, {:#x})",
        stext as usize,
        etext as usize
    );
}
