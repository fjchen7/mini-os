use super::{
    id::{kstack_alloc, KernelStack, TaskUserRes},
    process::ProcessControlBlock,
    TaskContext,
};
use crate::{
    mm::PhysPageNum,
    sync::{UPIntrFreeCell, UPIntrRefMut},
    trap::TrapContext,
};
use alloc::sync::{Arc, Weak};

// 线程控制块
pub struct TaskControlBlock {
    pub process: Weak<ProcessControlBlock>,
    pub kstack: KernelStack,
    // 存放运行时可变的元数据
    inner: UPIntrFreeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    // 线程独有的TID和用户栈
    pub res: Option<TaskUserRes>,
    // 存放Trap上下文存放的物理页号
    pub trap_cx_ppn: PhysPageNum,
    // 进程切换或时钟中断时，要保存的上下文
    pub task_cx: TaskContext,
    // 线程的执行状态
    pub task_status: TaskStatus,
    // 线程退出时，返回的退出码保存在这里
    pub exit_code: Option<i32>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    Ready,   // 就绪
    Running, // 运行
    Blocked, // 阻塞
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    #[allow(unused)]
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
}

impl TaskControlBlock {
    // 创建线程控制块
    pub fn new(
        process: Arc<ProcessControlBlock>,
        ustack_base: usize,
        alloc_user_res: bool,
    ) -> Self {
        // 分配线程的资源：TID、用户栈、存放TrapContext的内存
        let res = TaskUserRes::new(process.clone(), ustack_base, alloc_user_res);
        let trap_cx_ppn = res.trap_cx_ppn();
        // 分配线程的内核栈
        // 这里的实现，trap_cx和kstack的地址范围都在跳板之下，可能是重叠的。但它们分别位于进程和内核的地址空间中，不会冲突。
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        Self {
            process: Arc::downgrade(&process),
            kstack,
            inner: unsafe {
                UPIntrFreeCell::new(TaskControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                })
            },
        }
    }

    pub fn inner_exclusive_access(&self) -> UPIntrRefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    // 拿到所在进程的内存空间的token
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
}
