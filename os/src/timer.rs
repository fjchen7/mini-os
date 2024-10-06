use core::cmp::Ordering;

use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use crate::sync::UPIntrFreeCell;
use crate::task::{wakeup_task, TaskControlBlock};
use alloc::collections::binary_heap::BinaryHeap;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use riscv::register::time;

// TICKS_PER_SEC表示每秒里时钟中断发生的次数。
// 这里是每秒100次，所以时钟中断的间隔是10ms。
const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1_000;
const USEC_PER_SEC: usize = 1_000_000;

// 返回时间
// 这里读取了计数器寄存器mtime。它统计了上电以来，CPU经过的时钟周期数（这个时钟不同于“CPU时钟”，是专门用于计时的）
pub fn get_time() -> usize {
    time::read()
}

// 返回时间（毫秒）
// 计算CPU经过了多少秒：time(s) = get_time() / CLOCK_FREQ
// 这容易理解，用总的时钟周期数除以每秒的时钟周期数，就是经过的秒数。
// 可以类推，计算CPU经过了多少毫秒：time(ms) = get_time() / 每毫秒的时钟周期数
//                                     = get_time() / (CLOCK_FREQ / 1000)
pub fn get_time_ms() -> usize {
    time::read() / (CLOCK_FREQ / MSEC_PER_SEC)
}

// 返回时间（微秒）
#[allow(unused)]
pub fn get_time_us() -> usize {
    time::read() / (CLOCK_FREQ / USEC_PER_SEC)
}

// 设置下一个时钟中断（10ms后发生）
pub fn set_next_trigger() {
    let timer = get_time() + CLOCK_FREQ / TICKS_PER_SEC;
    set_timer(timer);
}

// 表示超时时间，用于唤醒阻塞的任务
pub struct TimerCondVar {
    // 若当前时间大于expire_ms时，则超时，可以唤醒任务
    pub expire_ms: usize,
    pub task: Arc<TaskControlBlock>,
}

impl PartialEq for TimerCondVar {
    fn eq(&self, other: &Self) -> bool {
        self.expire_ms == other.expire_ms
    }
}

impl Eq for TimerCondVar {}

impl PartialOrd for TimerCondVar {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimerCondVar {
    fn cmp(&self, other: &Self) -> Ordering {
        // core库提供的BinaryHeap是最大堆。但我们需要最小堆，所以这里反转了大小比较。
        self.expire_ms.cmp(&other.expire_ms).reverse()
    }
}

lazy_static! {
    // 用二插堆（优先队列）实现排序，每次从堆顶取出最小的时间
    static ref TIMERS: UPIntrFreeCell<BinaryHeap<TimerCondVar>> =
        unsafe { UPIntrFreeCell::new(BinaryHeap::<TimerCondVar>::new()) };
}

pub fn add_timer(expire_ms: usize, task: Arc<TaskControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    timers.push(TimerCondVar { expire_ms, task });
}

// 移除task所在的定时器。这在任务被唤醒时调用。
pub fn remove_timer(task: Arc<TaskControlBlock>) {
    let mut timers = TIMERS.exclusive_access();
    let mut temp = BinaryHeap::<TimerCondVar>::new();
    for condvar in timers.drain() {
        if Arc::as_ptr(&task) != Arc::as_ptr(&condvar.task) {
            temp.push(condvar);
        }
    }
    timers.clear();
    timers.append(&mut temp);
}

// 检查时间，唤醒超时的任务
pub fn check_timer() {
    let current_ms = get_time_ms();
    let mut timers = TIMERS.exclusive_access();
    while let Some(timer) = timers.peek() {
        if timer.expire_ms <= current_ms {
            wakeup_task(Arc::clone(&timer.task));
            timers.pop();
        } else {
            // 堆是有序的，所以后面的定时器不用再检查了
            break;
        }
    }
}
