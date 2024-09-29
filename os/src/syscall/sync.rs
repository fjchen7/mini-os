use alloc::sync::Arc;

use crate::{
    sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore},
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

// 为当前进程新增一个信号量。
// - res_count：该信号量的初始资源可用数量，为非负整数。
// - 返回值：假定该操作必定成功，返回创建的信号量的 ID。
pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let semaphore = Arc::new(Semaphore::new(res_count));
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(semaphore);
        id as isize
    } else {
        process_inner.semaphore_list.push(Some(semaphore));
        process_inner.semaphore_list.len() as isize - 1
    }
}

// 对当前进程的指定信号量进行 V 操作。
// - sem_id：信号量的 ID 。
// - 返回值：假定该操作必定成功，返回 0 。
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}

// 对当前进程的指定信号量进行 P 操作。
// - sem_id：信号量的 ID 。
// - 返回值：假定该操作必定成功，返回 0 。
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    0
}

// 为当前进程新增一个条件变量。
// - 返回值：假定该操作必定成功，返回创建的条件变量的 ID。
pub fn sys_condvar_create() -> isize {
    let condvar = Some(Arc::new(Condvar::new()));
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = condvar;
        id as isize
    } else {
        process_inner.condvar_list.push(condvar);
        process_inner.condvar_list.len() as isize - 1
    }
}

// 对当前进程的指定条件变量进行 signal 操作，即唤醒在该条件变量上阻塞的线程（如果存在）。
// - condvar_id：要操作的条件变量的 ID 。
// - 返回值：假定该操作必定成功，返回 0 。
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

// 对当前进程的指定条件变量进行 wait 操作，阶段分为：
// 1. 释放当前线程持有的一把互斥锁；
// 2. 阻塞当前线程，并将其加入指定条件变量的阻塞队列；
// 3. 等待其他线程用 signal 操作唤醒当前线程；
// 4. 重新获取之前持有的锁。
// - condvar_id：要操作的条件变量的 ID 。
// - mutex_id：当前线程持有的互斥锁的 ID 。
// - 返回值：假定该操作必定成功，返回 0 。
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
