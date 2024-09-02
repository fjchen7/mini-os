#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

#[no_mangle]
// 该程序仅打印字符，并退出。
fn main() -> i32 {
    println!("Hello, world!");
    0
}
