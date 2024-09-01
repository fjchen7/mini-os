#![no_std] // 开启no_std模式
#![no_main] // 禁用main函数作为入口点

mod lang_items;

use core::arch::global_asm;
// 载入汇编代码
global_asm!(include_str!("entry.asm"));

fn main() {}
