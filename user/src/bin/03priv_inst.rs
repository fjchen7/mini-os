#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use core::arch::asm;

#[no_mangle]
// 该程序执行RISC-V的特权指令sret，从User Mode（用户态）切换到Supervisor Mode（内核态）。
// 这会失败，因为用户态不允许直接S执行特权指令。
fn main() -> i32 {
    println!("Try to execute privileged instruction in U Mode");
    println!("Kernel should kill this application!");
    unsafe {
        asm!("sret");
    }
    0
}
