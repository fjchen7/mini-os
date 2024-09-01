# 这是汇编代码，编写进入内核的第一条指令。
    # 将后面的汇编代码，都放入.text.entry内存段中。
    # 常规的段名是.text，但我们想区分出入口代码，所以加了个.entry。
    .section .text.entry
    # 声明全局符号_start，作为程序的入口。
    .globl _start
_start:
    # 将boot_stack_top的地址存入sp寄存器。sp寄存器表示指向栈顶的指针。
    # la指令的全称是Load Address，用于将地址存入寄存器。
    la sp, boot_stack_top
    # 调用内核的入口函数rust_main，将控制权交给Rust代码。
    call rust_main

    # 声明段.bss.stack。该段会被链接脚本linker.ld放入.bss段中。
    .section .bss.stack
    # 声明全局符号boot_stack_lower_bound，表示栈的底部（内存高位）。
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    # 为栈预留空间 4096 * 16B = 64KB
    .space 4096 * 16
    # 声明全局符号boot_stack_top，表示栈的顶部（内存低位）。
    .globl boot_stack_top
boot_stack_top:
