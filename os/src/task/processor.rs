//!Implementation of [`Processor`] and Intersection of control flow
use super::manager::fetch_task;
use super::switch::__switch;
use super::task::TaskControlBlock;
use super::TaskContext;
use super::TaskStatus;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;

// 处理器单个核心的管理器，负责将从进程管理器中取出任务并执行
// 该结构表示CPU的执行状态，后续可扩展到多核系统
pub struct Processor {
    // 当前处理器正在运行的进程
    current: Option<Arc<TaskControlBlock>>,
    // idle控制流表示程序的空闲状态，此时没有进程在运行
    // 该字段保存处理器处于idle控制流时的任务上下文
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    // 获取当前正在运行的进程，并将其从处理器中移除
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    // 获取当前正在运行的进程
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

// 从idle控制流切换到任务控制流。idle控制流是两个任务之间的中间状态，用于解耦任务切出和切入。
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            // 取出当前处理器的idle控制流的任务上下文。这是要被替换的任务。
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // 从任务管理器中取出接下来要切换的任务
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            drop(task_inner);
            processor.current = Some(task);
            drop(processor);
            // 在__switch之前，都处于idle控制流中
            unsafe {
                // 切换任务
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
            // ⭐️ schedule方法最后的__switch，重新将idle控制流的上下文恢复到了寄存器里
            // 于是又会回到此处（上次idle控制流切出的地方），继续切换到下一个可运行的任务
        }
    }
}

// 将处理器切换到idle控制流状态，继续下一轮调度
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

// 获取当前正在运行的进程，并将其从处理器中移除
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

// 获取当前正在运行的进程
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_task_pid() -> usize {
    current_task().unwrap().getpid()
}

// 获取当前正在运行的进程的地址空间的token
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

// 获取当前正在运行的进程的TrapContext
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}
