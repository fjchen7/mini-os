use alloc::sync::Arc;

use crate::{
    mm::kernel_token,
    task::{add_task, current_task, TaskControlBlock},
    trap::{trap_handler, TrapContext},
};

// 在当前进程里，创建一个新的线程
// - entry：线程的入口函数地址
// - arg：入口函数的参数。0 表示没有参数。
// - 返回值：创建的线程的 TID
// 内核会为每个线程分配专属于该线程的资源：用户栈、Trap上下文、内核栈
// 前面两个在进程地址空间中，内核栈在内核地址空间中。
pub fn sys_thread_create(entry: usize, arg: usize) -> isize {
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    // 创建新线程
    let new_task = Arc::new(TaskControlBlock::new(
        Arc::clone(&process),
        task.inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .ustack_base,
        true,
    ));
    // 将新线程加入任务队列
    add_task(Arc::clone(&new_task));
    let new_task_inner = new_task.inner_exclusive_access();
    let new_task_res = new_task_inner.res.as_ref().unwrap();
    let new_task_tid = new_task_res.tid;
    let mut process_inner = process.inner_exclusive_access();
    // 将新线程加入到当前进程中
    let tasks = &mut process_inner.tasks;
    while tasks.len() < new_task_tid + 1 {
        tasks.push(None);
    }
    tasks[new_task_tid] = Some(Arc::clone(&new_task));
    // 组装新线程的Trap上下文
    let new_task_trap_cx = new_task_inner.get_trap_cx();
    *new_task_trap_cx = TrapContext::app_init_context(
        entry,
        new_task_res.ustack_top(),
        kernel_token(),
        new_task.kstack.get_top(),
        trap_handler as usize,
    );
    new_task_trap_cx.x[10] = arg;
    new_task_tid as isize
}

// 获取当前线程的 TID
pub fn sys_gettid() -> isize {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid as isize
}

// 等待线程退出
// - tid：指定线程的 TID
// - 返回值：
//   - -1：线程不存在
//   - -2：线程还没退出
//   - 其他：该线程结束的退出码
pub fn sys_waittid(tid: usize) -> i32 {
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let task_inner = task.inner_exclusive_access();
    let mut process_inner = process.inner_exclusive_access();
    // 线程不能等待自己结束
    if task_inner.res.as_ref().unwrap().tid == tid {
        return -1;
    }
    let mut exit_code: Option<i32> = None;
    let waited_task = process_inner.tasks[tid].as_ref();
    if let Some(waited_task) = waited_task {
        if let Some(waited_exit_code) = waited_task.inner_exclusive_access().exit_code {
            exit_code = Some(waited_exit_code);
        }
    } else {
        // 等待的线程不存在
        return -1;
    }
    if let Some(exit_code) = exit_code {
        // 释放线程资源（TID、用户栈、存放Trap上下文的内存）
        process_inner.tasks[tid] = None;
        exit_code
    } else {
        // 等待的线程还没退出
        -2
    }
}
