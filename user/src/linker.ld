/* 布局与第1章的链接脚本（见os/src/linker.ld）差不多，但有以下修改
   - 起始地址改成0x80400000，这是Linux系统加载ELF可执行文件的默认起始地址（系统则从0x80200000开始加载）
   - 不需要记录.text、.rodata、.data段的起始和结束地址。
   - 但需要记录.bss段的起始和结束地址，才能在程序启动时清零。
   - .text.entry会放置_start函数（定义在lib.rs中）的指令 */

OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80400000;

SECTIONS
{
    . = BASE_ADDRESS;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    .bss : {
        start_bss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        end_bss = .;
    }
    /DISCARD/ : {
        *(.eh_frame)
    }
}
