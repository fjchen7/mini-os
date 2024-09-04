use crate::task::TASK_MANAGER;

// 退出程序
pub fn sys_exit(exit_code: i32) -> ! {
    println_kernel!("Application exited with code {}", exit_code);
    TASK_MANAGER.exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

// 程序主动让出CPU，调度到其他应用
pub fn sys_yield() -> isize {
    TASK_MANAGER.suspend_current_and_run_next();
    0
}
