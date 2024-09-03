# 开启宏的功能
.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    # 以下内容放进.text段
    .section .text
    # 声明全局符号，供其他文件引用
    .globl __alltraps
    .globl __restore
    # 4字节对齐，这是RISC-V特权级的规范要求
    .align 2

# 该函数的功能：内核如何处理Trap。
# 当前的寄存器代表程序的上下文，需要保存到栈（内核栈）上。
__alltraps:
    # 在内核初始化后，sscratch会指向内核栈栈顶。

    # 指令csrrw rd, csr, rc的功能：
    #   - 将csr的值写入通用寄存器rd
    #   - 将通用寄存器rd的值写入csr
    # CSR全称Control and Status Register，是控制和描述某些特权级行为的寄存器。这里就是描述Trap状态。
    # 下面的指令，相当于交换sp和sscratch中的值
    # 该指令之前，sscratch指向内核栈，sp指向用户栈
    # 该指令之后，sscratch指向用户栈，sp指向内核栈
    csrrw sp, sscratch, sp

    # 保存Trap上下文到内核栈上
    # 要保存的内容：通用寄存器x0~x31，sstatus和sepc
    # 需要分配34*8字节的栈帧给内核栈，这正是Rust定义的TrapContext的大小
    # 栈从高位往低位增长，所以这里是减34*8
    addi sp, sp, -34*8

    # 保存x0~x31。实际上不保存这几个：
    #   - x0：固定为0，不需要保存
    #   - tp（x4）：一般不会被用到
    #   - sp（x2）：当前正在使用。后面再保存。
    sd x1, 1*8(sp)
    sd x3, 3*8(sp)
    # 使用类似循环的功能.rept，保存剩下的寄存器x5~x31。
    # 每一步都执行宏SAVE_GP
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr

    # 保存sscratch/spec到内核栈上
    # RISC-V中，数据处理和访指操作，只能使用通用寄存器。因此这里要先写入t0/t1，再写入内核栈。
    # t0（x5）/t1（x6）这两个寄存器，在上面都已经保存，可以随便使用。
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # 将sscratch的值保存到内核栈的sp（x2）上。
    # sscratch表示用户栈的地址。系统调用结束后，需要恢复。
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # 将sp中的值写入寄存器a0，作为参数（&TrapContext）传递给trap_handler函数
    # 寄存器a0~17（x10~x17）用于传递函数参数。
    mv a0, sp
    # 调用trap_handler函数（Rust中定义）
    call trap_handler

# 该函数的功能：恢复程序上下文，并返回到用户态。
# 所谓的恢复上下文，就是将保存了程序状态的寄存器值的某个栈，读到寄存器中。
# 该函数用在：
# 1）内核处理完Trap后。
# 2）内核刚开始运行时。此时调用该函数，才能进入用户态，执行第一个程序。
__restore:
    # trap_handler函数返回后，将TrapContext的地址作为返回值
    # 函数的返回值会保存到寄存器a0中
    # 因此这里相当于，将内核栈的地址赋值给sp
    mv sp, a0

    # 从内核栈中恢复sstatus、sepc和sscratch
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    ld t2, 2*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    csrw sscratch, t2

    # 在此处，sp指向内核栈，sscratch指向用户栈

    # 恢复通用寄存器
    # 依旧不用管x0，tp（x4）和sp（x2）
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr

    # 释放此次系统调用的内核栈（TrapContext）
    # 栈顶缩小，向高地址增长
    addi sp, sp, 34*8

    # 交换sp和sscratch的值
    # 该指令之后，sscratch指向内核栈，sp指向用户栈
    csrrw sp, sscratch, sp

    # 执行Supervisor级特权指令sret，将控制流还给程序
    # sret指令会做：
    #   - 将CSR寄存器spec中的值写入pc寄存器。程序会跳转到pc指向地址的指令上执行。
    #   - 根据sstatus寄存器的值，切换到特权级。0/1分别切换到User/Supervisor。
    # 在调用__store的方法run_next_app中，spec的值被设置为APP_BASE_ADDRESS，也就是程序的起始地址0x80400000。
    sret
