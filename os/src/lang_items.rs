use core::panic::PanicInfo;

use crate::{println, sbi::shutdown};

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
    shutdown(true)
}
