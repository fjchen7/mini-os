// 进程管理
// - 全局变量`TASK_MANAGER`管理整个系统的进程队列
// - 全局变量`PROCESSOR`管理处理器的单个核如何调度进程
// - 全局变量`PID_ALLOCATOR`管理进程ID的分配

mod action;
mod context;
mod manager;
mod pid;
mod processor;
mod signal;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::fs::open_file;
use crate::fs::OpenFlags;
use crate::sbi::shutdown;
use alloc::sync::Arc;
use lazy_static::*;
use manager::remove_from_pid2task;
use task::{TaskControlBlock, TaskStatus};

pub use action::SignalAction;
pub use manager::add_task;
pub use manager::pid2task;
pub use pid::pid_alloc;
pub use processor::{
    current_task, current_task_pid, current_trap_cx, current_user_token, run_tasks, schedule,
    take_current_task,
};
pub use signal::{SignalFlags, MAX_SIG};

pub use context::TaskContext;

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // 获取当前正在运行的任务
    let task = take_current_task().unwrap();
    // 获取当前任务的TaskContext
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // 修改当前任务的状态
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    // 将任务重新加入到任务管理器中
    add_task(task);
    // 进入调度逻辑
    schedule(task_cx_ptr);
}

pub const IDLE_PID: usize = 0;

// 退出当前进程，并运行下一个进程
pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();
    let pid = task.getpid();

    // 如果是idle进程退出，则直接关机
    if pid == IDLE_PID {
        println_kernel!("Idle process exit with exit_code {} ...", exit_code);
        let failure = exit_code != 0;
        shutdown(failure);
    }
    remove_from_pid2task(pid);

    let mut inner = task.inner_exclusive_access();
    // 将要退出的进程的状态设置为Zombie
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    // 将该任务的所有子进程，都移交给initproc
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
        inner.children.clear();
    }

    // 回收分配给该进程的物理页。
    // 这是子进程成为僵尸进程后，先回收的部分资源。剩余未回收的资源，由父进程或initproc进程回收。
    inner.memory_set.recycle_data_pages();
    // write back dirty pages
    for mapping in inner.file_mappings.iter() {
        mapping.sync();
    }
    drop(inner);
    drop(task);

    // 进入调度逻辑。该_unused变量，实际就是Processor下的idle_task_cx。
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    // 全局的initproc进程，用来初始化用户shell
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

// 将initproc添加到任务管理器中
pub fn add_initproc() {
    add_task(INITPROC.clone());
}

pub fn check_signals_error_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.signals.check_error()
}

// 将一个要处理的信号，加到当前的进程中
pub fn current_add_signal(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.signals |= signal;
}

// 由内核处理的信号
fn call_kernel_signal_handler(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            task_inner.frozen = true;
            // 将SIGSTOP从待处理的信号集合中移除
            task_inner.signals ^= SignalFlags::SIGSTOP;
        }
        SignalFlags::SIGCONT => {
            if task_inner.signals.contains(SignalFlags::SIGCONT) {
                // 将SIGCONT从待处理的信号集合中移除
                task_inner.signals ^= SignalFlags::SIGCONT;
                task_inner.frozen = false;
            }
        }
        _ => {
            task_inner.killed = true;
        }
    }
}

// 由用户进程处理的信号
fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let handler = task_inner.signal_actions.table[sig].handler;
    if handler != 0 {
        // 标记当前信号正在处理
        task_inner.handling_sig = sig as isize;
        // 将当前要处理的信号，从待处理的信号集合中移除
        task_inner.signals ^= signal;

        // 保存进入信号处理逻辑前的上下文
        let trap_ctx = task_inner.get_trap_cx();
        task_inner.trap_ctx_backup = Some(*trap_ctx);

        // 设置信号处理逻辑的函数入口
        trap_ctx.sepc = handler;
        // 设置参数（a0）为信号编码
        trap_ctx.x[10] = sig;
        // 这里为了实现方便，没有修改sp，因此信号处理逻辑还是在当前的用户栈上执行
        // Linux则会为每次信号处理函数，分配新的用户栈
    } else {
        // 如果程序没有自定义处理该信号的逻辑，使用默认行为（直接忽略）
        println_kernel!(
            "task/call_user_signal_handler {}: default action: ignore it or kill process",
            sig
        );
    }
}

// 检查收到的信号，并对它们进行处理
fn check_pending_signals() {
    for sig in 0..(MAX_SIG + 1) {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        if task_inner.signals.contains(signal) && (!task_inner.signal_mask.contains(signal)) {
            let mut masked = true;
            // 检查该即将要处理的信号，是否被当前正在处理的信号屏蔽
            let handling_sig = task_inner.handling_sig;
            if handling_sig == -1 {
                // 如果当前不在处理其他信号，则没有信号屏蔽
                masked = false;
            } else {
                // 如果当前在处理其他信号，则检查当前信号是否屏蔽了该即将要处理的信号
                let handling_sig = handling_sig as usize;
                if !task_inner.signal_actions.table[handling_sig]
                    .mask
                    .contains(signal)
                {
                    masked = false;
                }
            }
            if !masked {
                drop(task_inner);
                drop(task);
                if signal == SignalFlags::SIGKILL
                    || signal == SignalFlags::SIGSTOP
                    || signal == SignalFlags::SIGCONT
                    || signal == SignalFlags::SIGDEF
                {
                    // 上面4个信号只能由内核处理
                    call_kernel_signal_handler(signal);
                } else {
                    // 其余信号交由程序处理
                    call_user_signal_handler(sig, signal);
                    return;
                }
            }
        }
    }
}

// 信号的处理入口
pub fn handle_signals() {
    loop {
        // 真正处理信号的逻辑在check_pending_signals里
        check_pending_signals();
        let (frozen, killed) = {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            (task_inner.frozen, task_inner.killed)
        };
        // 如果没被挂起（由SIGSTOP触发），或者被杀死，则退出循环
        if !frozen || killed {
            break;
        }
        // 如果被挂起（frozen==true），则走到这里：切换到其他进程，等待它们发送SIGCONT恢复当前进程
        // 后续将继续在这个loop里继续循环，直到收到SIGCONT信号
        // 这个loop只是为了处理SIGSTOP/SIGCONT信号这个情况
        suspend_current_and_run_next();
    }
}
