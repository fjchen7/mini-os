use super::TaskContext;
use core::arch::global_asm;

global_asm!(include_str!("switch.S"));

extern "C" {
    // 当前正在执行的任务是current_task。此时的寄存器，就是它的上下文。
    // 这个函数：1）将此时的寄存器状态，保存到current_task_cx_ptr中；
    //         2）将next_task_cx_ptr中存的上下文，加载到寄存器中。这包括：
    //              - ra：指向__restore方法
    //              - sp：指向保存TrapContext的用户栈
    //         3）执行ret指令，它将ra的值写入到pc寄存器中，跳转到__restore方法。
    //         4）在__restore中，找到sp所指向的TrapContext，将其恢复到寄存器中。最终会将pc寄存器恢复
    //              -
    pub fn __switch(current_task_cx_ptr: *mut TaskContext, next_task_cx_ptr: *const TaskContext);
}
