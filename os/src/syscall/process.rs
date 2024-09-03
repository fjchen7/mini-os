//! 应用程序管理相关的系统调用
use crate::batch::run_next_app;

// 退出程序
pub fn sys_exit(exit_code: i32) -> ! {
    println_kernel!("Application exited with code {}", exit_code);
    run_next_app()
}
