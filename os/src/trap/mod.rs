mod context;

use crate::{syscall::syscall, task::TASK_MANAGER};
use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Trap},
    stval, stvec,
};

global_asm!(include_str!("trap.S"));

// 设置中断处理函数的入口地址
pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        // CSR寄存器stvec存放中断处理代码的地址，即我们在trap.S中定义的__alltraps的地址。
        // 它有两个模式Direct和Vectored，我们选择Direct模式。
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

#[no_mangle]
// 处理中断、异常或来自用户态的系统调用
// 该方法由汇编方法__alltraps调用，参数cx也是它构造的（直接在栈上构造的）。
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read(); // 拿到Trap的发生原因
    let stval = stval::read(); // 拿到Trap发生时的附加信息
    match scause.cause() {
        // 系统调用。用户程序调用ecall指令时，会触发该类型的异常。
        Trap::Exception(Exception::UserEnvCall) => {
            // CSR寄存器sepc，记录Trap发生之前执行的最后一条指令地址（即ecall指令）。
            // 需要让sepc指向下一条指令，以便系统调用返回后，继续执行用户态的指令。
            cx.sepc += 4;
            // 从寄存器x17中读取系统调用号，从x10, x11, x12中读取参数。
            // 执行系统调用，并将结果写回x10。
            // x10，x11，x12，x17，又名a0，a1，a2，a7
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
        }
        // 访存异常
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {
            println_kernel!("PageFault in application, kernel killed it.");
            TASK_MANAGER.exit_current_and_run_next();
        }
        // 非法指令
        Trap::Exception(Exception::IllegalInstruction) => {
            println_kernel!("IllegalInstruction in application, kernel killed it.");
            TASK_MANAGER.exit_current_and_run_next();
        }
        // 暂时不支持的Trap类型
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub use context::TrapContext;
