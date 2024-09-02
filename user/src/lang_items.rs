use core::panic::PanicInfo;

use crate::println;

// 自定义`panic!`的行为。它必须在`#![no_std]`应用程序中定义。
// `#[panic_handler]`必须放在函数`fn(info: &PanicInfo) -> !`上
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let err = info.message().unwrap();
    if let Some(location) = info.location() {
        println!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            err,
        );
    } else {
        println!("Panicked: {}", err);
    }
    // 内核遇到panic，会直接关机。
    // 而应用程序遇到panic，不会导致内核崩溃关机。这是我们实现的特权级机制的体现。
    loop {}
}
