// 进程管理
// - 全局变量`TASK_MANAGER`管理整个系统的进程队列
// - 全局变量`PROCESSOR`管理处理器的单个核如何调度进程
// - 全局变量`PID_ALLOCATOR`管理进程ID的分配

mod context;
mod manager;
mod pid;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use core::ops::AddAssign;

use crate::loader::{get_app_data, get_num_app};
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::timer::get_time_us;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use task::{TaskControlBlockInner, TaskStatus};

pub use context::TaskContext;
