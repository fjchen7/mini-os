use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_ref, translated_refmut, translated_str},
    task::{
        current_process, current_task, current_task_pid, current_user_token,
        exit_current_and_run_next, pid2process, suspend_current_and_run_next, SignalAction,
        SignalFlags, MAX_SIG,
    },
    timer::get_time_ms,
};
use alloc::{string::String, sync::Arc, vec::Vec};

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
    let process = current_process();
    if let Some(old_brk) = process.change_program_brk(size) {
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
    let process = current_process();

    let mut inner = process.inner_exclusive_access();
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
    let current_process = current_process();
    let new_process = current_process.fork();
    let new_pid = new_process.getpid();

    // 获取子进程的主线程的Trap上下文。这是子进程的第一个任务。
    let new_process_inner = new_process.inner_exclusive_access();
    let task = new_process_inner.tasks[0].as_ref().unwrap();
    let trap_cx = task.inner_exclusive_access().get_trap_cx();

    // 我们需要将子进程的fork返回值设为0，才能区分父子进程。返回值的地址在a0寄存器中。
    // x[10]就是a0寄存器
    trap_cx.x[10] = 0;
    new_pid as isize
}

// 将程序加载到当前进程的地址空间，并开始执行。
// - path：该程序的名字，系统能通过它找到其ELF二进制数据。从根目录找。
// - args：参数列表。类型为字符串数组，每个元素是一个字符串的起始地址。
// - 返回值：执行成功则不返回，失败则返回-1。
pub fn sys_exec(path: *const u8, mut args: *const usize) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let mut args_vec: Vec<String> = Vec::new();
    loop {
        let arg_str_ptr = *translated_ref(token, args);
        if arg_str_ptr == 0 {
            break;
        }
        let arg_str = translated_str(token, arg_str_ptr as *const u8);
        args_vec.push(arg_str);
        unsafe {
            args = args.add(1);
        }
    }
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let data = app_inode.read_all();
        let process = current_process();
        let argc = args_vec.len();
        process.exec(data.as_slice(), args_vec);
        argc as isize // 这个返回值会被赋给x[10]
    } else {
        -1
    }
}

// Linux内核规定，不允许对信号SIGKILL和SIGSTOP自定义处理逻辑
fn check_sigaction_error(signal: SignalFlags, action: usize, old_action: usize) -> bool {
    action == 0
        || old_action == 0
        || signal == SignalFlags::SIGKILL
        || signal == SignalFlags::SIGSTOP
}

// 为当前进程注册信号处理函数
// - signum：信号的编号
// - action：要注册的信号处理函数的指针
// - old_action：保存原先的信号处理函数的指针
// - 返回值：成功返回0，失败返回-1（如信号类型不存在，action 或 old_action 为空指针）
pub fn sys_sigaction(
    signum: i32,
    action: *const SignalAction,
    old_action: *mut SignalAction,
) -> isize {
    let token = current_user_token();
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if signum as usize > MAX_SIG {
        return -1;
    }
    if let Some(flag) = SignalFlags::from_bits(1 << signum) {
        if check_sigaction_error(flag, action as usize, old_action as usize) {
            return -1;
        }
        let prev_action = inner.signal_actions.table[signum as usize];
        *translated_refmut(token, old_action) = prev_action;
        // 注意，action不能跨页。要通过16字节对齐来保证。
        inner.signal_actions.table[signum as usize] = *translated_ref(token, action);
        0
    } else {
        -1
    }
}

// 设置当前进程的全局信号掩码。
// - mask：信号掩码，每一位代表一个信号，1表示屏蔽，0表示不屏蔽。
// - 返回值：成功返回原先的信号掩码，失败返回-1（如传参错误）
// syscall ID: 135
pub fn sys_sigprocmask(mask: u32) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    let old_mask = inner.signal_mask;
    if let Some(flag) = SignalFlags::from_bits(mask) {
        inner.signal_mask = flag;
        old_mask.bits() as isize
    } else {
        -1
    }
}

// 通知内核，进程的信号处理程序退出，可以恢复正常的执行流
// - 返回值：成功返回0，失败返回-1
pub fn sys_sigreturn() -> isize {
    let process = current_process();
    let task = current_task().unwrap();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.handling_sig = -1;
    let inner = task.inner_exclusive_access();
    // 恢复进程信号处理逻辑前，保存的Trap上下文
    let trap_ctx = inner.get_trap_cx();
    *trap_ctx = process_inner.trap_ctx_backup.unwrap();
    // Here we return the value of a0 in the trap_ctx,
    // otherwise it will be overwritten after we trap
    // back to the original execution of the application.
    trap_ctx.x[10] as isize
}

/// 向进程（可以是自身）发送信号。
/// - pid：接受信号的进程的PID
/// - signum：要发送的信号的编号。
/// - 返回值：成功返回0，失败返回-1（如进程或信号类型不存在）
pub fn sys_kill(pid: usize, signum: i32) -> isize {
    if let Some(process) = pid2process(pid) {
        if let Some(flag) = SignalFlags::from_bits(1 << signum) {
            let mut task_ref = process.inner_exclusive_access();
            if task_ref.signals.contains(flag) {
                return -1;
            }
            // 实现很简单，就将信号插入到进程控制块的signals字段
            task_ref.signals.insert(flag);
            0
        } else {
            -1
        }
    } else {
        -1
    }
}
