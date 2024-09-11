// 进程管理
// - 全局变量`TASK_MANAGER`管理整个系统的进程队列
// - 全局变量`PROCESSOR`管理处理器的单个核如何调度进程
// - 全局变量`PID_ALLOCATOR`管理进程ID的分配

mod context;
mod manager;
mod pid;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use core::ops::AddAssign;

use crate::loader::{get_app_data, get_app_data_by_name, get_num_app};
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
pub use manager::add_task;
pub use processor::{
    current_task, current_task_pid, current_trap_cx, current_user_token, schedule,
    take_current_task,
};
use task::{TaskControlBlock, TaskControlBlockInner, TaskStatus};

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
    // 进入调度逻辑。？？？？？？？？？
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

// 退出当前进程，并运行下一个进程
pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();

    // 如果是idle进程退出，则直接关机
    if task.getpid() == IDLE_PID {
        println_kernel!("Idle process exit with exit_code {} ...", exit_code);
        let failure = exit_code != 0;
        shutdown(failure);
    }

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
    drop(inner);
    drop(task);

    // 进入调度逻辑。该_unused变量，实际就是Processor下的idle_task_cx。
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    // 全局的initproc进程，用来初始化用户shell
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("initproc").unwrap()
    ));
}

// 将initproc添加到任务管理器中
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
