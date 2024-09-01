use core::panic::PanicInfo;

// 自定义`panic!`的行为。它必须在`#![no_std]`应用程序中定义。
// `#[panic_handler]`必须放在函数`fn(info: &PanicInfo) -> !`上
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
