use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

// 自旋式的互斥锁
// 在锁被占用时，将不断尝试获取锁。这将占用CPU时间。
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                // 如果阻塞，就切换时间片
                // 但下次调度时，会继续尝试获取锁
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

// 阻塞式的互斥锁
// 在锁被占用时，会将该任务设置为阻塞状态，不再调度它。
// 操作系统检查到锁可用后，将唤醒该任务，使其获得锁。
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    // 想获得该锁的任务队列
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new() -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            // 将线程设置为阻塞状态后，就不再调度它
            block_current_and_run_next();
        } else {
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            // 当前线程释放锁后，唤醒等待队列中的第一个线程
            // 这相当于，释放锁的线程，将锁直接移交给该被唤醒的线程
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
    }
}
