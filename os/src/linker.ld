/* 这是链接脚本，用于指定如何将多个目标文件链接成一个可执行文件。
   链接脚本需要定义内存布局、段的分布、入口等信息。
   我们使用Qemu模拟RISC-V计算机，它不支持动态链接。因此我们采用静态链接的编译方式。 */

/* 设置目标架构为riscv */
OUTPUT_ARCH(riscv)
/* 指定程序入口为符号_start（在汇编代码entry.asm中定义） */
ENTRY(_start)
/* 定义常量BASE_ADDRESS，表示内核代码的初始地址（也叫基地址） */
BASE_ADDRESS = 0x80200000;

/* 合并目标文件的内存段 */
SECTIONS
{
    /* .表示当前地址，赋值为BASE_ADDRESS */
    . = BASE_ADDRESS;
    /* 将当前地址.赋值给符号skernel，表示内核的开始地址 */
    skernel = .;

    /* .text段的开始位置 */
    stext = .;

    /* 定义.text段，存放代码的二进制。
       这里主要确定，按什么顺序将目标文件的哪些段放进去 */
    .text : {
        /* 将.text.entry段（汇编代码entry.asm中定义）放到开头。
           这样就能确保第一条指令在BASE_ADDRESS处，能被Qemu正确加载。*/        */
        *(.text.entry)
        /* 将其他目标文件的.text段和.text.*段放到后面 */
        *(.text .text.*)
    }
    /* 将当前地址对齐到4KB */
    . = ALIGN(4K);
    /* .text段的结束位置 */
    etext = .;

    /* .rodata段的开始位置 */
    srodata = .;
    /* 定义.rodata段，存放只读数据 */
    .rodata : {
        /* 将所有目标文件的.rodata段和.rodata.*段放到这里 */
        *(.rodata .rodata.*)
        /* 将所有目标文件的.srodata段和.srodata.*段放到这里 */
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    /* .rodata段的结束位置 */
    erodata = .;

    /* .data段的开始位置 */
    sdata = .;  /* .data段的开始位置 */
    /* 定义.data段，存放读写数据 */
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    . = ALIGN(4K);
    /* .data段的结束位置 */
    edata = .;

    /* 定义.bss段，存放未初始化的数据 */
    .bss : {
        *(.bss.stack)
        sbss = .;  /* .bss段的开始位置 */
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    /* .bss段的结束位置 */
    ebss = .;

    /* 内核的结束位置 */
    ekernel = .;

    /* 要丢弃的段，不会被包含进可执行文件中。这里只丢弃.eh_frame段 */
    /DISCARD/ : {
        *(.eh_frame)
    }
}
