use crate::{
    loader::get_app_data_by_name,
    mm::translated_str,
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, take_current_task,
    },
    timer::get_time_ms,
};

// 退出程序
pub fn sys_exit(exit_code: i32) -> ! {
    let current_task = take_current_task().unwrap();
    println_kernel!("PID {} exited with code {}", current_task.pid.0, exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

// 程序主动让出CPU，调度到其他应用
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

// 返回CPU时间（毫秒）
pub fn sys_get_time() -> isize {
    get_time_ms() as isize
}

// 增加或减少堆的大小。返回旧的堆顶地址。
// brk表示堆顶指针，称为program break。
pub fn sys_sbrk(size: i32) -> isize {
    todo!("fix")
    // if let Some(old_brk) = TASK_MANAGER.change_current_program_brk(size) {
    //     old_brk as isize
    // } else {
    //     -1
    // }
}

// 等待子进程变成僵尸进程后，回收全部资源并返回
// - pid：要等待的子进程的PID，-1表示等待任意子进程；
// - exit_code：保存子进程的返回值的地址，为0表示不保存。
// - 返回值：
//   - -1：等待的子进程不存在；
//   - -2：等待的子进程均未结束；
//   - 其他：结束的子进程的PID。
//
// 目前该系统调用是阻塞的，会一直等待直到有子进程结束。
pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    todo!()
}

// 复制出一个子进程
// 返回值：当前进程返回子进程的PID，子进程则返回0
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // 我们需要将子进程的fork返回值设为0，才能区分父子进程。返回值的地址在a0寄存器中。
    trap_cx.x[10] = 0; // x[10]就是a0寄存器
                       // 将子进程加入任务队列
    add_task(new_task);
    new_pid as isize
}

// 将程序加载到当前进程的地址空间，并开始执行。
// - path：该程序的名字，系统能通过它找到其ELF二进制数据
// - 返回值：执行成功则不返回，失败则返回-1。
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0 // 这个返回值没有意义，因为在exec方法里，我们已经重新初始化了Trap上下文
    } else {
        -1
    }
}
