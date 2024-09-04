use super::TaskContext;

#[derive(Copy, Clone)]
// 用于描述任务的状态和上下文
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
}

#[derive(Copy, Clone, PartialEq)]
// 将任务分成四个状态
pub enum TaskStatus {
    UnInit,  // 未初始化
    Ready,   // 准备运行
    Running, // 正在运行
    Exited,  // 已退出
}
