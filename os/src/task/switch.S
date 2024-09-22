.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch

# 程序A运行，中断触发，陷入内核态，执行中断逻辑，再返回到用户态。
# 处于该过程的内核态时，会保存上下文：处理结束后要返回的地址（ra）、当前任务的内核栈的栈顶指针（sp）。
#
# 这个函数想做：在内核态处理完程序A的中断后，不回到程序A，而是进入程序B的执行流。
#
# 前面提到的ra/sp，决定了中断结束后返回哪里（ra），以及切换回去必要的信息（sp）。
# 因此，只要将ra/sp等寄存器替换为程序B的，并在某处保存程序A的，就可以实现任务切换。
#
# 该函数的流程：
# 1）保存当前任务A的上下文（当前寄存器ra/sp的值）到current_task_cx_ptr
# 2）将任务B的上下文（next_task_cx_ptr）恢复到寄存器中
# 3）执行ret指令，跳转到ra寄存器中的地址，执行指令。
#
# 结构TaskContext，记录了任务陷入内核态里要保存的ra/sp。
# TaskContext:
#     ra: usize,       // 此次中断处理结束后，要返回的地址
#     sp: usize,       // 本任务的内核栈的栈顶指针
#     s: [usize; 12],  // 寄存器s0~s11
# 函数签名：
# __switch(
#     current_task_cx_ptr: *mut TaskContext,
#     next_task_cx_ptr: *const TaskContext
# )
__switch:
    # a0：current_task_cx_ptr
    # a1：next_task_cx_ptr
    # 当前正在运行的是任务A，此时的寄存器是它的上下文

    # 保存任务A的上下文到current_task_cx_ptr
    # 1）保存ra到current_task_cx_ptr->ra
    # 寄存器ra是当前任务完成后的返回地址
    sd ra, 0(a0)
    # 2）保存sp到current_task_cx_ptr->sp
    # 寄存器sp是当前任务的内核栈的栈顶指针
    sd sp, 8(a0)
    # 3）保存s0~s20到current_task_cx_ptr->s
    .set n, 0
    .rept 12
        SAVE_SN %n
        .set n, n + 1
    .endr

    # 切换到任务B，需要恢复它的上下文到当前的寄存器
    ld ra, 0(a1)
    ld sp, 8(a1)
    .set n, 0
    .rept 12
        LOAD_SN %n
        .set n, n + 1
    .endr

    # ret指令用于从函数中返回，将控制权交还给调用者
    # 它会读出ra寄存器的值，写入pc寄存器。程序将跳转到pc所指地址的指令上执行，从而做到返回的效果
    ret
