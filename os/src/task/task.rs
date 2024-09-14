use core::cell::RefMut;

use super::{
    pid::{KernelStack, PidHandle},
    pid_alloc, TaskContext,
};
use crate::{
    config::TRAP_CONTEXT,
    fs::{File, Stdin, Stdout},
    mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE},
    sync::UPSafeCell,
    trap::{trap_handler, TrapContext},
};
use alloc::{
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};

// 单个进程的控制块
// 进程的执行状态、资源控制等元数据，都保存在该结构体中。
pub struct TaskControlBlock {
    // 不可变
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    // inner存放运行时可变的元数据
    inner: UPSafeCell<TaskControlBlockInner>,
}

pub struct TaskControlBlockInner {
    // Trap上下文存放的物理页号。它的虚拟页是地址空间的次高页。
    pub trap_cx_ppn: PhysPageNum,
    // 该进程的地址空间中，从0x0到用户栈结束所包含的字节。
    // 它代表了该进程占用的内存大小（暂时不包含堆）。
    pub base_size: usize,
    // 进程切换或时钟中断时，要保存的上下文
    pub task_cx: TaskContext,
    // 进程的执行状态
    pub task_status: TaskStatus,
    // 地址空间
    pub memory_set: MemorySet,
    // 父进程。使用Weak，避免循环引用。
    pub parent: Option<Weak<TaskControlBlock>>,
    // 子进程
    pub children: Vec<Arc<TaskControlBlock>>,
    // 进程退出时，返回的退出码保存在这里
    pub exit_code: i32,
    // 文件描述符表
    // 元素所在的下标就是文件描述符。如果元素为None，则表示该文件描述符未被使用，可以重新被分配。
    pub fd_table: Vec<Option<Arc<dyn File + Send + Sync>>>,

    // 堆的底部，即堆的起始地址。数字小（堆从低地址向高地址增长）。
    pub heap_bottom: usize,
    // 堆的顶部，即堆的结束地址。数字大。
    // 这个指针的名字就叫program break。
    pub program_brk: usize,
}

#[derive(Copy, Clone, PartialEq)]
// 任务的状态
pub enum TaskStatus {
    Ready,   // 准备运行
    Running, // 正在运行
    Zombie,  // 僵尸进程：进程已经结束，但父进程还没有回收它的资源
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
    // 分配一个文件描述符
    pub fn alloc_fd(&mut self) -> usize {
        if let Some(fd) = (0..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
            fd
        } else {
            self.fd_table.push(None);
            self.fd_table.len() - 1
        }
    }
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }

    pub fn getpid(&self) -> usize {
        self.pid.0
    }

    fn init_fd_table() -> Vec<Option<Arc<dyn File + Send + Sync>>> {
        vec![
            // 0 -> stdin
            Some(Arc::new(Stdin)),
            // 1 -> stdout
            Some(Arc::new(Stdout)),
            // 2 -> stderr
            Some(Arc::new(Stdout)),
        ]
    }

    // 解析ELF格式的二进制数据，创建一个新的进程
    pub fn new(elf_data: &[u8]) -> Self {
        // 解析ELF，得到地址空间、用户栈顶、入口地址
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 得到存放TrapContext的物理页号。该物理页在前面的from_elf中已经分配。
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 分配新的PID
        let pid_handle = pid_alloc();
        // 在内核地址空间中，为该PID分配内核栈
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // 初始化TaskContext，使得第一次切换到该进程时，能跳转到trap_return方法，进入它的用户态
        let task_cx = TaskContext::goto_trap_return(kernel_stack_top);
        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx,
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table: Self::init_fd_table(),
                    heap_bottom: user_sp,
                    program_brk: user_sp,
                })
            },
        };
        // 在用户空间，初始化该进程的TrapContext
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }

    // 从父进程复制出一个子进程
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner_exclusive_access();
        // 为子进程分配新的地址空间
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        // 为子进程分配新的PID和内核栈
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        // 复制父进程的fd
        let fd_table = parent_inner.fd_table.clone();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    fd_table,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                })
            },
        });
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        // 更新父进程的children
        parent_inner.children.push(task_control_block.clone());
        task_control_block
    }

    // 申请新的地址空间，加载ELF文件，将替换原来的地址空间。同时初始化TrapContext。
    pub fn exec(&self, elf_data: &[u8]) {
        // 申请新的地址空间，加载ELF文件
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();

        let mut inner = self.inner_exclusive_access();
        // 将该进程块的内存空间替换为新的内存空间
        inner.memory_set = memory_set;
        inner.trap_cx_ppn = trap_cx_ppn;
        inner.base_size = user_sp;
        inner.heap_bottom = user_sp;
        inner.program_brk = user_sp;
        inner.fd_table = Self::init_fd_table();
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
    }

    // 增加或减少堆的大小
    // 改变成功时，返回原来堆的结束位置（最高位）
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner_exclusive_access();
        let old_break = inner.program_brk;
        let new_brk = inner.program_brk as isize + size as isize;
        if new_brk < inner.heap_bottom as isize {
            return None;
        }
        let heap_bottom = VirtAddr(inner.heap_bottom);
        let new_end = VirtAddr(new_brk as usize);
        let result = if size < 0 {
            inner.memory_set.shrink_to(heap_bottom, new_end)
        } else {
            inner.memory_set.append_to(heap_bottom, new_end)
        };
        if result {
            inner.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }
}
