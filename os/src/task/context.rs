use crate::trap::trap_return;

#[derive(Copy, Clone)]
#[repr(C)]
// 程序陷入Trap后，要记录一些上下文，才能在Trap处理完后，回到该程序，继续执行。
// 这类似函数的栈帧，都表示在做了某次调用后，回到原地方的所需信息。
// 在内核态里，恢复哪个任务的上下文，就能回到它的用户态。该结构被__switch方法使用。
pub struct TaskContext {
    // Trap处理完后，要跳到哪个指令上执行。
    // 它会指向用于返回到用户态的函数，比如__restore/trap_return。
    ra: usize,
    // 属于本任务的内核栈栈顶指针（低位）
    // 该寄存器的意义，是给ra指向的方法提供参数。
    sp: usize,
    // 被调用者需要保存的寄存器s0-s11（对应x8~x9，x18~x27）
    // 对于一般函数，Rust/C编译器会为其生成代码，来保存s0~s11寄存器。
    // 但__switch是汇编函数，所以要手动处理。
    s: [usize; 12],
    // 其他寄存器不用保存，是因为：
    // - 调用者负责保存的寄存器，由编译器为函数生成；
    // - 临时寄存器，不需要保存和恢复。
}

impl TaskContext {
    // 初始化TaskContext
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    // 设置该任务在内核栈的栈顶指针（sp），并将程序的返回地址（ra）设为trap_return方法
    // 这用于任务的初始化：第一次启动时，就能进入__restore方法，从而进入用户态执行该程序。
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
