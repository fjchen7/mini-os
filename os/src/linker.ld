/* 这是链接脚本，用于定义内核的内存布局、符号、入口等信息。
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

    /* 定义.text段，存放代码的二进制 */
    stext = .;  /* .text段的开始位置 */
    .text : {
        /* 将.text.entry段（entry.asm中定义）放到开头。
           这样就能确保第一条指令在BASE_ADDRESS处，能被Qemu正确加载。*/
        *(.text.entry)
        /* 对齐，让Trap的处理方法__alltraps（trap.S中定义），刚好位于该物理页的开头 */
        . = ALIGN(4K);
        strampoline = .;
        *(.text.trampoline);
        . = ALIGN(4K);
        /* 将其他目标文件的.text段和.text.*段放到后面 */
        *(.text .text.*)
    }
    . = ALIGN(4K);  /* 对齐到4KB */
    etext = .;  /* .text段的结束位置 */

    /* 定义.rodata段，存放只读数据 */
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);
    erodata = .;

    /* 定义.data段，存放读写数据 */
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    . = ALIGN(4K);
    edata = .;

    /* 定义.bss段，存放未初始化的数据 */
    sbss_with_stack = .;
    .bss : {
        *(.bss.stack)
        sbss = .;  /* .bss段的开始位置 */
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    . = ALIGN(4K);
    ebss = .;

    ekernel = .;  /* 内核的结束位置 */

    /* 要丢弃的段，不会被包含进可执行文件中。这里只丢弃.eh_frame段 */
    /DISCARD/ : {
        *(.eh_frame)
    }
}
