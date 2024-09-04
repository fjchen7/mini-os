.altmacro
.macro SAVE_SN n
    sd s\n, (\n+2)*8(a0)
.endm
.macro LOAD_SN n
    ld s\n, (\n+2)*8(a1)
.endm
    .section .text
    .globl __switch

# 该函数的功能：切换任务。
# 1. 保存当前寄存器到current_task_cx_ptr（当前正在运行的任务就是current_task）
# 2. 将next_task_cx_ptr中存放的上下文恢复到寄存器中
# 3. 执行ret指令，跳转到ra寄存器中的地址执行指令
# 函数签名：
# __switch(
#     // 当前任务A的用户栈，用来保存上下文
#     current_task_cx_ptr: *mut TaskContext,
#     // 即将切换过去的任务B的用户栈
#     next_task_cx_ptr: *const TaskContext
# )
# TaskContext:
#     ra: usize,       // 本任务完成后的返回地址
#     sp: usize,       // 本任务的内核栈的栈顶指针
#     s: [usize; 12],  // 寄存器s0~s11
__switch:
    # a0表示current_task_cx_ptr
    # a1表示next_task_cx_ptr
    # 当前正在运行的是任务A，此时的寄存器是它的上下文

    # 保存任务A的上下文到它的用户栈current_task_cx_ptr
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
    # 它会将ra寄存器中的值，写入pc寄存器。程序将跳转到pc指向地址的指令上执行，从而做到返回的效果
    ret
