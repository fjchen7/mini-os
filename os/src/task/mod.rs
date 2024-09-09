// 任务管理
// 使用全局变量`TASK_MANAGER`来管理内核中的任务

mod context;
mod switch;

#[allow(clippy::module_inception)]
mod task;

use core::ops::AddAssign;

use crate::loader::{get_app_data, get_num_app};
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;

// 任务管理器（的外壳）
pub struct TaskManager {
    // 任务总数
    num_app: usize,
    // 为了让TaskManager能作为全局静态变量，且同时能被修改，我们又得使用UPSafeCell来获得内部可变性
    inner: UPSafeCell<TaskManagerInner>,
    // 统计切换时间
    switch_time: UPSafeCell<usize>,
}

// 真正的任务管理器...
pub struct TaskManagerInner {
    // 任务列表
    tasks: Vec<TaskControlBlock>,
    // 当前正在运行的任务的id
    current_task: usize,
}

lazy_static! {
    // 用于管理任务的全局变量
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            let elf = get_app_data(i);
            tasks.push(TaskControlBlock::new(elf, i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
            switch_time: unsafe { UPSafeCell::new(0) },
        }
    };
}

impl TaskManager {
    unsafe fn switch(
        &self,
        current_task_cx_ptr: *mut TaskContext,
        next_task_cx_ptr: *const TaskContext,
    ) {
        let star_time = get_time_us();
        switch::__switch(current_task_cx_ptr, next_task_cx_ptr);
        let duration = get_time_us() - star_time;
        self.switch_time.exclusive_access().add_assign(duration)
    }

    // 运行第一个任务
    pub fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;
        drop(inner); // 内部用了RefCell做到内部可变性。所以要提前drop，保证安全。
        let mut _unused = TaskContext::zero_init();
        // 将第一个任务的上下文加载到当前环境（寄存器）中，并开始执行。
        // 由于前面没有任务运行，所以这里直接丢掉当前保存的上下文（这里的unused）
        unsafe {
            self.switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        // __switch修改了pc寄存器，不会执行到这里
        panic!("unreachable in run_first_task!");
    }

    // 将当前正在运行的任务的状态，从Running改为Ready
    pub fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    // 将当前正在运行的任务的状态，从Running改为Exited
    pub fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    // 找到下一个要运行的任务。在这里，返回第一个Ready状态的任务。
    pub fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    // 将当前任务，切换到下一个任务执行。如果没有Ready状态的任务，就关机。
    pub fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            // 内部用了RefCell做到内部可变性。所以要提前drop，保证安全。
            drop(inner);
            // 将当前任务的上下文保存到current_task_cx_ptr中
            // 然后加载next_task_cx_ptr的上下文到寄存器，并开始运行next_task
            unsafe {
                self.switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // 此处会回到用户态
        } else {
            println!("All applications completed!");
            println!(
                "task switch time: {} us",
                self.switch_time.exclusive_access()
            );
            shutdown(false);
        }
    }

    // 挂起当前任务，并运行下一个任务
    // 这里会将当前任务的状态改成Ready。它能被run_next_task方法重新找到，再次执行。
    pub fn suspend_current_and_run_next(&self) {
        self.mark_current_suspended();
        self.run_next_task();
    }

    // 退出当前任务，并运行下一个任务
    pub fn exit_current_and_run_next(&self) {
        self.mark_current_exited();
        self.run_next_task();
    }

    // 返回当前任务的地址空间所对应的satp寄存器的值
    pub fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_user_token()
    }

    // 返回当前任务的TrapContext
    pub fn get_current_trap_cx(&self) -> &mut TrapContext {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].get_trap_cx()
    }
}
