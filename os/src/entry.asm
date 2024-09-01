# 这是汇编代码，编写进入内核的第一条指令。
    # 将后面的汇编代码，都放入.text.entry内存段中。
    # 常规的段名是.text，但我们想区分出入口代码，所以加了个.entry。
    .section .text.entry
    # 声明全局符号_start，作为程序的入口。
    .globl _start
_start:
    # 将立即数100存入寄存器x1。
    # li是指令的名字，全称Load Immediate。
    li x1, 100
