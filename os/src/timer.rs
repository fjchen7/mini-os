use crate::config::CLOCK_FREQ;
use crate::sbi::set_timer;
use riscv::register::time;

// TICKS_PER_SEC表示每秒里时钟中断发生的次数。
// 这里是每秒100次，所以时钟中断的间隔是10ms。
const TICKS_PER_SEC: usize = 100;
const MSEC_PER_SEC: usize = 1_000;

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

// 设置下一个时钟中断（10ms后发生）
pub fn set_next_trigger() {
    let timer = get_time() + CLOCK_FREQ / TICKS_PER_SEC;
    set_timer(timer);
}
