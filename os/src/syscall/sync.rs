use alloc::sync::Arc;

use crate::{
    sync::{Mutex, MutexBlocking, MutexSpin},
    task::{block_current_and_run_next, current_process, current_task},
    timer::{add_timer, get_time_ms},
};

// 使当前线程睡眠一段时间。
// - sleep_ms：睡眠的时间，单位为毫秒。
// - 返回值： 0
pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// 为当前进程新增一把互斥锁。
// - blocking：true 表示基于阻塞的互斥锁，不会占用CPU，等待操作系统通知；
//            false 表示基于自旋的互斥锁，会占用CPU，不断尝试获取锁。
// - 返回值：假设该操作必定成功，返回创建的锁的 ID。
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    // 从进程的互斥锁列表中，找到一个空位，或者添加一个新的互斥锁
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}

// 当前线程尝试获取所属进程的一把互斥锁。
// - mutex_id：要获取的锁的 ID 。
// - 返回值： 0
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    0
}

// 当前线程释放所属进程的一把互斥锁。
// - mutex_id：要释放的锁的 ID 。
// - 返回值： 0
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
