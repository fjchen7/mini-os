use crate::{
    config::{kernel_stack_position, TRAP_CONTEXT},
    mm::{MapPermission, MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE},
    trap::{trap_handler, TrapContext},
};

use super::TaskContext;

// 用于控制任务的结构体
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    // 地址空间
    pub memory_set: MemorySet,
    // Trap上下文存放的物理页。它的虚拟页是地址空间的次高页。
    pub trap_cx_ppn: PhysPageNum,
    // 应用数据的大小，也就是地址空间中，从0x0到用户栈结束所包含的字节
    #[allow(unused)]
    pub base_size: usize,
}

#[derive(Copy, Clone, PartialEq)]
// 任务的状态
pub enum TaskStatus {
    Ready,   // 准备运行
    Running, // 正在运行
    Exited,  // 已退出
}

impl TaskControlBlock {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }

    // 解析ELF格式的二进制数据，创建一个TaskControlBlock
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // 解析ELF，得到地址空间、用户栈顶、入口地址
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        // 得到存放TrapContext的物理页号
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // 在内核地址空间中，为该程序专属的内核栈，分配物理页。
        // 使用物理页分配器（这里的类型是Framed）。
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        // 初始化TaskContext
        let task_cx = TaskContext::goto_trap_return(kernel_stack_top);
        let task_control_block = Self {
            task_status,
            task_cx,
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
        };
        // 初始化程序的TrapContext
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
}
