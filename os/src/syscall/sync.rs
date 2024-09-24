use crate::{
    task::{block_current_and_run_next, current_task},
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
