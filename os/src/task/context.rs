#[derive(Copy, Clone)]
#[repr(C)]
// 任务的上下文，类似函数的栈帧。
pub struct TaskContext {
    // 本任务完成后，要返回的地址（如__restore）。
    // 即汇编方法__switch执行完该任务并返回，要跳到哪里继续执行
    ra: usize,
    // 本任务的内核栈的栈顶指针（最低位）
    // 这个栈区域保存该任务/程序的TrapContext
    sp: usize,
    // 被调用者需要保存的寄存器s0-s11（对应x8~x9，x18~x27）
    // 对于一般函数，Rust/C编译器会为其生成代码，来保存s0~s11寄存器。
    // 但__switch是汇编函数，所以要手动处理。
    s: [usize; 12],
    // 其他寄存器不用保存，是因为：
    // - 有一些是属于调用者保存的寄存器，由编译器为函数生成；
    // - 有一些是临时寄存器，不需要保存和恢复。
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

    // 设置该任务在内核栈中的栈顶指针sp，并将程序的返回地址（ra）设置为__restore方法
    pub fn goto_restore(kstack_ptr: usize) -> Self {
        extern "C" {
            fn __restore();
        }
        Self {
            ra: __restore as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
