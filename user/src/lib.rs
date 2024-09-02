#![no_std]
#![feature(panic_info_message)]

#[macro_use]
pub mod console;
mod lang_items;

#[no_mangle]
fn main() -> i32 {
    panic!("Cannot find main!");
}
