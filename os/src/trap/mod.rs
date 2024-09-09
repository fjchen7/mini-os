mod context;

use crate::{
    config::{TRAMPOLINE, TRAP_CONTEXT},
    syscall::syscall,
    task::TASK_MANAGER,
    timer::set_next_trigger,
};
use core::arch::{asm, global_asm};
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

// 设置中断处理函数的入口地址
pub fn init() {
    set_kernel_trap_entry();
}

fn set_kernel_trap_entry() {
    unsafe {
        stvec::write(trap_from_kernel as usize, TrapMode::Direct);
    }
}

fn set_user_trap_entry() {
    unsafe {
        // CSR寄存器stvec存放Trap处理函数的地址。它有两个模式Direct和Vectored。
        // __alltraps的地址是物理地址。
        // 开启了分页后，就只能用虚拟地址，也就是这里的TRAMPOLINE。它映射到的物理地址，就是__alltraps
        stvec::write(TRAMPOLINE, TrapMode::Direct);
    }
}

#[no_mangle]
// 暂时不考虑在内核态触发Trap的情况。第9章会涉及。
pub fn trap_from_kernel() -> ! {
    todo!("a trap from kernel!");
}

// 初始化时钟中断
pub fn enable_timer_interrupt() {
    unsafe {
        // 设置sie.stie为1，使得时钟中断不会被屏蔽
        sie::set_stimer();
    }
}

#[no_mangle]
// 处理中断、异常或来自用户态的系统调用
// 该方法由汇编方法__alltraps调用，参数cx也是它构造的（直接在栈上构造的）。
pub fn trap_handler() -> ! {
    // 简单起见，不考虑在内核态触发Trap的情况，直接panic。
    set_kernel_trap_entry();
    // 执行该方法时，系统处于内核的地址空间中。
    // 但应用的TrapContext保存在应用的地址空间中，没法以参数形式传给trap_handler。
    let cx = TASK_MANAGER.get_current_trap_cx(); // 拿到TrapContext
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
        // 时钟中断
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            TASK_MANAGER.suspend_current_and_run_next();
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
    trap_return();
}

#[no_mangle]
// 设定参数，进入__restore方法，回到用户态
pub fn trap_return() -> ! {
    // 重新设置Trap处理函数的入口地址
    set_user_trap_entry();
    let trap_cx_ptr = TRAP_CONTEXT;
    let user_satp = TASK_MANAGER.get_current_token();
    extern "C" {
        fn __alltraps();
        fn __restore();
    }
    // 得到__restore的虚拟地址
    // TRAMPOLINE是__alltraps的虚拟地址，因此通过偏移量就能算出来。
    let restore_va = TRAMPOLINE + (__restore as usize - __alltraps as usize);
    unsafe {
        asm!(
            // CPU有指令缓存（i-cache）
            // 内核的某些操作，可能导致映射了.text段的物理页变化，使得缓存非法
            // fence.i是内存屏障指令，用来清空指令缓存i-cache
            "fence.i",
            "jr {restore_va}",             // 跳转到__restore。下面设定参数a0和a1。
            restore_va = in(reg) restore_va,
            in("a0") trap_cx_ptr,      // a0 = Trap上下文的虚拟地址
            in("a1") user_satp,        // a1 = 程序地址空间的根页表地址
            options(noreturn)
        );
    }
    panic!("Unreachable in back_to_user!");
}

pub use context::TrapContext;
