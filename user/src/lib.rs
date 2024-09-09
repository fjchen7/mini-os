#![no_std]
#![feature(panic_info_message)]
#![feature(linkage)] // 支持弱链接的标记

#[macro_use]
pub mod console;
mod lang_items;
pub mod syscall;

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
    // 下面要进入的main方法，由应用程序各自实现。链接生成ELF时，会替换此处的默认main实现。
    let exist_code = main();
    syscall::sys_exit(exist_code);
    panic!("unreachable after sys_exit!");
}
