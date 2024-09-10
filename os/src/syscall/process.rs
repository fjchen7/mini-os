use crate::{task::TASK_MANAGER, timer::get_time_ms};

// 退出程序
pub fn sys_exit(exit_code: i32) -> ! {
    let task_id = TASK_MANAGER.get_current_task_id();
    println_kernel!("App {} exited with code {}", task_id, exit_code);
    TASK_MANAGER.exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

// 程序主动让出CPU，调度到其他应用
pub fn sys_yield() -> isize {
    TASK_MANAGER.suspend_current_and_run_next();
    0
}

// 返回CPU时间（毫秒）
pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

// 增加或减少堆的大小。返回旧的堆顶地址。
// brk表示堆顶指针，称为program break。
pub fn sys_sbrk(size: i32) -> isize {
    if let Some(old_brk) = TASK_MANAGER.change_current_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
