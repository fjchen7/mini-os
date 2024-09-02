// RustSBI可执行Machine级别的RISC-V指令。它提供的接口规范称为SBI。
// 内核需要通过SBI，才能获得这些特权指令的执行权限。
// 这里用到了库sbi_rt提供的SBI接口的封装。

// 打印字符
pub fn console_putchar(c: usize) {
    #[allow(deprecated)]
    sbi_rt::legacy::console_putchar(c);
}

// 关机
// failure为false时，表示正常关机
pub fn shutdown(failure: bool) -> ! {
    use sbi_rt::{system_reset, NoReason, Shutdown, SystemFailure};
    if !failure {
        system_reset(Shutdown, NoReason);
    } else {
        system_reset(Shutdown, SystemFailure);
    }
    unreachable!()
}
