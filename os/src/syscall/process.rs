use alloc::sync::Arc;

use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_task_pid, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_ms,
};

// 退出程序
pub fn sys_exit(exit_code: i32) -> ! {
    let pid = current_task_pid();
    println_kernel!("PID {} exited with code {}", pid, exit_code);
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
    let current_task = current_task().unwrap();
    if let Some(old_brk) = current_task.change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

// 返回当前进程的PID
pub fn sys_getpid() -> isize {
    current_task_pid() as isize
}

// 找到当前进程的僵尸子进程，回收全部资源
// - pid：要找的子进程PID，-1表示等待任意子进程；
// - exit_code：保存子进程的返回值的地址，为0表示不保存。
// - 返回值：
//   - -1：找不到对应的子进程；
//   - -2：等待的子进程均未退出；
//   - 其他：结束的子进程的PID。
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();

    let mut inner = task.inner_exclusive_access();
    // 如果找不到对应的子进程，返回-1
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
    }

    // 找到一个僵尸子进程
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
    });

    // 回收该僵尸子进程的资源
    if let Some((idx, _)) = pair {
        // 从父进程的子进程列表中移除
        let child = inner.children.remove(idx);
        assert_eq!(Arc::strong_count(&child), 1); // 保证它没有其他引用
        let found_pid = child.getpid();
        // 保存子进程的返回值到exit_code_ptr所指向的地址
        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
}

// 复制出一个子进程
// 返回值：当前进程返回子进程的PID，子进程则返回0
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // 我们需要将子进程的fork返回值设为0，才能区分父子进程。返回值的地址在a0寄存器中。
    // x[10]就是a0寄存器
    trap_cx.x[10] = 0;
    // 将子进程加入任务队列
    add_task(new_task);
    new_pid as isize
}

// 将程序加载到当前进程的地址空间，并开始执行。
// - path：该程序的名字，系统能通过它找到其ELF二进制数据。从根目录找。
// - 返回值：执行成功则不返回，失败则返回-1。
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(data.as_slice());
        0 // 这个返回值没有意义，因为在exec方法里，我们已经重新初始化了Trap上下文
    } else {
        -1
    }
}
