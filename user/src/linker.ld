/* 程序的（虚拟）内存布局 */

OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x10000;

SECTIONS
{
    . = BASE_ADDRESS;
    .text : {
        /* .text.entry放置入口方法_start（定义在lib.rs中）的指令 */
        *(.text.entry)
        *(.text .text.*)
    }
    /* 需要对齐，是因为相邻两个逻辑段的访问权限不同 */
    . = ALIGN(4K);
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    /* .data和.bss的访问权限相同，无需再对齐 */
    .bss : {
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    /DISCARD/ : {
        *(.eh_frame)
        *(.debug*)
    }
}
