mod context;

use crate::{
    config::{PAGE_SIZE, TRAMPOLINE},
    mm::VirtAddr,
    syscall::syscall,
    task::{
        check_signals_error_of_current, current_add_signal, current_process, current_task_pid,
        current_trap_cx, current_trap_cx_user_va, current_user_token, exit_current_and_run_next,
        handle_signals, suspend_current_and_run_next, SignalFlags,
    },
    timer::set_next_trigger,
};
use alloc::sync::Arc;
use core::{
    arch::{asm, global_asm},
    cmp::min,
};
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
    panic!("a trap {:?} from kernel!", scause::read().cause())
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
    let scause = scause::read(); // 拿到Trap的发生原因
    let stval = stval::read(); // 拿到Trap发生时的附加信息
    match scause.cause() {
        // 系统调用。用户程序调用ecall指令时，会触发该类型的异常。
        Trap::Exception(Exception::UserEnvCall) => {
            // CSR寄存器sepc，放着Trap发生之前执行的最后一条指令地址（即ecall指令）。
            // 需要让sepc移动4字节，指向下一条指令，以便系统调用返回后，继续执行用户态的指令。
            let mut cx = current_trap_cx();
            cx.sepc += 4;
            // 从寄存器x17中读取系统调用号，从x10, x11, x12中读取参数。
            // 执行系统调用，并将结果写回x10。
            // x10，x11，x12，x17，又名a0，a1，a2，a7
            let result = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]);
            // sys_exec会替换掉当前任务的Trap上下文。因此要重新拿一遍。
            cx = current_trap_cx();
            cx.x[10] = result as usize;
        }
        // 时钟中断
        Trap::Interrupt(Interrupt::SupervisorTimer) => {
            set_next_trigger();
            suspend_current_and_run_next();
        }
        // 访存异常
        Trap::Exception(Exception::StoreFault)
        | Trap::Exception(Exception::StorePageFault)
        | Trap::Exception(Exception::InstructionFault)
        | Trap::Exception(Exception::InstructionPageFault)
        | Trap::Exception(Exception::LoadFault)
        | Trap::Exception(Exception::LoadPageFault) => {
            current_add_signal(SignalFlags::SIGSEGV);
            if !handle_page_fault(stval) {
                println_kernel!(
                    "PageFault {:?} in PID {}, bad addr = {:#x}, bad instruction = {:#x}, killed by kernel.",
                    scause.cause(),
                    current_task_pid(),
                    stval,
                    current_trap_cx().sepc);
                current_add_signal(SignalFlags::SIGSEGV);
            }
        }
        // 非法指令
        Trap::Exception(Exception::IllegalInstruction) => {
            println_kernel!(
                "IllegalInstruction in PID {}, killed by kernel.",
                current_task_pid()
            );
            current_add_signal(SignalFlags::SIGILL);
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
    // 处理信号
    handle_signals();
    // 如果检查到错误信号，就退出当前进程，切换到下一个进程
    if let Some((errno, msg)) = check_signals_error_of_current() {
        println_kernel!("signal error {}", msg);
        exit_current_and_run_next(errno);
    }
    trap_return();
}

#[no_mangle]
// 引入地址空间后，TrapContext放在了用户的地址空间中。
//
// __restore的任务，是从TrapContext中恢复上下文，回到用户态。
// 为了让它能找到TrapContext，需要提供两个参数：
// - TrapContext的虚拟地址
// - 地址空间，即页表地址（satp寄存器的值）。
//
// trap_return的任务，就是提供这两个值作参数，并跳转到__restore。
pub fn trap_return() -> ! {
    // 先前进入Trap时，关闭了Trap处理函数的入口地址。现在重新设置它为__alltraps
    set_user_trap_entry();
    // 拿到执行__restore需要两个参数：TrapContext的虚拟地址和地址空间
    let trap_cx_user_va = current_trap_cx_user_va();
    let user_satp = current_user_token();
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
            in("a0") trap_cx_user_va,  // a0 = Trap上下文的虚拟地址
            in("a1") user_satp,        // a1 = 程序地址空间的根页表地址
            options(noreturn)
        );
    }
}

// 延迟加载mmap的文件映射到内存。将加载fault_addr所在的整个页。
pub fn handle_page_fault(fault_addr: usize) -> bool {
    let fault_va: VirtAddr = fault_addr.into();
    let fault_vpn = fault_va.floor();
    let process = current_process();
    let mut pcb = process.inner_exclusive_access();

    // 如果页表中已经有映射，那么不能处理
    if let Some(pte) = pcb.memory_set.translate(fault_vpn) {
        if pte.is_valid() {
            return false;
        }
    }

    match pcb.file_mappings.iter_mut().find(|m| m.contains(fault_va)) {
        Some(mapping) => {
            let file = Arc::clone(&mapping.file);
            // 延迟加载，访问时才分配物理页。且如果之前已经映射过，那么不会再次分配物理页，共享之前的物理页。
            let (ppn, range, shared) = mapping.map(fault_va).unwrap();
            // 更新页表
            // pcb.memory_set.map(fault_vpn, ppn, range.perm);
            // 如果不是共享的（分配了新的物理页），则从文件中读取数据
            // 这是mmap的功能，即映射文件内容到内存
            if !shared {
                // 如果先前mmap映射了[0, 100)的虚拟地址到文件的[100, 200)的内容
                // 此时访问虚拟地址为50的内容，那就会加载[50, 100)的内容到物理页（假设页大小超过50）
                let file_size = file.size() as usize;
                let file_offset = range.file_offset(fault_vpn);
                assert!(file_offset < file_size);
                // 加载内容不超过一个页
                let read_len = min(PAGE_SIZE, file_size - file_offset);
                file.read_at(file_offset, &mut ppn.get_bytes_array()[..read_len]);
            }
            true
        }
        None => false,
    }
}

pub use context::TrapContext;
