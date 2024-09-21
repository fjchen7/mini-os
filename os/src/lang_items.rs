use core::panic::PanicInfo;

use crate::{println, sbi::shutdown};
use core::arch::asm;

// 自定义`panic!`的行为。它必须在`#![no_std]`应用程序中定义。
// `#[panic_handler]`必须放在函数`fn(info: &PanicInfo) -> !`上
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        println!("Panicked: {}", info.message().unwrap());
    }
    unsafe {
        print_stack_trace();
    }
    shutdown(true)
}

// 打印函数的调用栈
pub unsafe fn print_stack_trace() {
    let mut fp: *const usize;
    let stop = current_kstack_top();
    asm!("mv {}, fp", out(reg) fp);
    println!("\u{1B}[31m[{}]\u{1B}[0m", "---START BACKTRACE---");
    let mut i = 0;
    while !fp.is_null() && *fp != stop {
        let saved_ra = *fp.sub(1);
        let saved_fp = *fp.sub(2);

        println!(
            "\u{1B}[31m{:2}:\u{1B}[0m 0x{:016x}, fp = 0x{:016x}",
            i, saved_ra, saved_fp
        );

        i += 1;
        fp = saved_fp as *const usize;
    }
    println!("\u{1B}[31m[{}]\u{1B}[0m", "---END   BACKTRACE---");
}
