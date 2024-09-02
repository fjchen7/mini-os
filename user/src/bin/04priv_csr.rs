#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use riscv::register::sstatus::{self, SPP};

#[no_mangle]
// 该程序尝试在用户态，修改内核态的控制状态寄存器（CSR, Control and Status Register），即sstatus变量。
// 这会失败，因为用户态不允许直接访问内核态的CSR。
fn main() -> i32 {
    println!("Try to access privileged CSR in U Mode");
    println!("Kernel should kill this application!");
    unsafe {
        sstatus::set_spp(SPP::User);
    }
    0
}
