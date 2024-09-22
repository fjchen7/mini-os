# 开启宏的功能
.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm

    # 以下汇编代码放进.text.trampoline段
    .section .text.trampoline
    # 声明全局符号，供其他文件引用
    .globl __alltraps
    .globl __restore
    # 4字节对齐，这是RISC-V特权级的规范要求
    .align 2

# 该函数的功能：内核如何处理Trap。
# 当前的寄存器代表程序的上下文，需要保存到栈（内核栈）上。
__alltraps:
    # 进入该方法时
    # - sscratch是程序的地址空间中，当前线程的Trap上下文的保存位置。
    #   该值是由__restore方法设置的。
    # - sp则指向用户栈

    # 指令csrrw rd, csr, rc的功能：
    #   - 将csr的值写入通用寄存器rd
    #   - 将通用寄存器rd的值写入csr
    # CSR全称Control and Status Register，是控制和描述某些特权级行为的寄存器。这里用于描述Trap状态。
    # 下面的指令，相当于交换sp和sscratch中的值
    # 该指令之后，sscratch指向用户栈，sp指向TrapContext要保存的地址。两者都在该程序的地址空间里。
    csrrw sp, sscratch, sp

    # 保存Trap上下文到该栈上
    # 要保存的内容：通用寄存器x0~x31，sstatus和sepc
    # 需要分配34*8字节的栈帧给内核栈，这正是Rust定义的TrapContext的大小
    # 栈从高位往低位增长，现在sp的值为KERNEL_STACK[i] - size_of(TrapContext)
    ############ addi sp, sp, -34*8

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

    # 保存sscratch/spec的值
    # RISC-V中，数据处理和访指操作，只能使用通用寄存器。因此这里要先写入t0/t1，再写入内核栈。
    # t0（x5）/t1（x6）这两个寄存器，在上面都已经保存，可以随便使用。
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)

    # 保存用户栈的值
    csrr t2, sscratch
    sd t2, 2*8(sp)

    # 恢复出内核地址空间所对应的satp寄存器的值到t0
    ld t0, 34*8(sp)
    # 恢复出Trap的处理方法的地址到t1
    ld t1, 36*8(sp)
    # 切换到当前程序所对应的内核栈栈顶
    ld sp, 35*8(sp)

    # 切换到内核的地址空间
    csrw satp, t0
    # 刷新TLB
    sfence.vma

    # 跳转到t1存储的地址，即Trap的处理方法。jr表示jump register。
    # 不能使用call trap_handler，是因为call指令实际是让pc加上一个偏移量。
    # 而由于设计的缘故。我们将跳板（该代码所在的内存区域名称）设置为虚拟地址空间的最高页，通过pc偏移无法访问到。
    jr t1

# 该函数的功能：恢复程序的Trap上下文，并返回到用户态。
# 所谓的恢复上下文，就是将保存了程序状态的寄存器值的某个栈，读到寄存器中。
__restore:
    # 该方法有两个参数，分别存在寄存器：
    # - a0：在该程序的地址空间中，Trap上下文的保存位置。对所有程序，该值都是相同的。
    # - a1：该程序的所对应的satp寄存器的值（也叫token）。

    # 切换到程序的地址空间
    csrw satp, a1
    # 刷新TLB
    sfence.vma

    # 原本sscratch指向用户栈，a0指向TrapContext的保存位置。两者都在该程序的地址空间中。
    # 现在交换sp和sscratch的值
    csrw sscratch, a0
    # 切换到用户栈上。栈顶是TrapContext。
    mv sp, a0

    # 开始恢复程序的Trap上下文
    # 恢复sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1

    # 恢复通用寄存器。依旧不用管x0，tp（x4）和sp（x2）
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr

    # 恢复进入Trap前，用户栈的值
    ld sp, 2*8(sp)

    # 执行Supervisor级特权指令sret，将控制流还给程序
    # sret指令会做：
    #   - 将CSR寄存器spec中的值写入pc寄存器。程序会跳转到pc指向地址的指令上执行。
    #   - 根据sstatus寄存器的值，切换到特权级。0/1分别切换到User/Supervisor。
    sret
