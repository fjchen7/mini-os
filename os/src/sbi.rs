// RustSBI可执行Machine级别的RISC-V指令。它提供的接口规范称为SBI。
// 内核需要通过SBI，才能获得这些特权指令的执行权限。
// 这里用到了库sbi_rt提供的SBI接口的封装。

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

// 设置下一个时钟中断
pub fn set_timer(timer: usize) {
    // 设置了mtimecmp寄存器的值。
    // 一旦计数器寄存器mtime超过mtimecmp的值，就会触发时钟中断。
    sbi_rt::set_timer(timer as _);
}
