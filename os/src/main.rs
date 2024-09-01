#![no_std] // 开启no_std模式
#![no_main] // 禁用main函数作为入口点

mod lang_items;
mod sbi;

use core::arch::global_asm;
// 载入汇编代码
global_asm!(include_str!("entry.asm"));

// 不同的编译器，在编译时，会修改函数/变量的符号名，以做到解决命名冲突或增加类型安全等。
// 这叫做name mangling，不同的编译器有自身的策略。
//
// #[no_mangle]的作用：告诉编译器不修改函数的符号（name mangling）
// 我们要在汇编代码里调用Rust函数，因此要确保函数符号不被修改。否则，汇编代码将找不到该函数。
#[no_mangle]
pub fn rust_main() -> ! {
    // 内核初始化
    clear_bss();
    loop {}
}

// 清零bss段（未初始化的全局变量）
fn clear_bss() {
    // 从链接器脚本（linker.ld）获取内存段的地址。通过引用外部的C函数来获取它们。
    extern "C" {
        fn sbss(); // .bss段的起始地址
        fn ebss(); // .bss段的结束地址
    }
    // 将.bss段清零
    (sbss as usize..ebss as usize).for_each(|a| unsafe {
        (a as *mut u8).write_volatile(0)
    })
}
