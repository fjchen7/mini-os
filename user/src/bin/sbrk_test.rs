#![no_std]
#![no_main]

#[macro_use]
extern crate user_lib;

use core::ptr::slice_from_raw_parts_mut;
use user_lib::syscall::sys_sbrk;

#[no_mangle]
fn main() -> i32 {
    println!("Test sys_sbrk start.");
    const PAGE_SIZE: usize = 0x1000;
    let origin_brk = sys_sbrk(0);
    println!("origin break point = {:x}", origin_brk);
    let brk = sys_sbrk(PAGE_SIZE as i32);
    if brk != origin_brk {
        return -1;
    }
    let brk = sys_sbrk(0);
    println!("one page allocated,  break point = {:x}", brk);
    println!("try write to allocated page");
    let new_page = unsafe {
        &mut *slice_from_raw_parts_mut(origin_brk as usize as *const u8 as *mut u8, PAGE_SIZE)
    };
    for pos in 0..PAGE_SIZE {
        new_page[pos] = 1;
    }
    println!("write ok");
    sys_sbrk(PAGE_SIZE as i32 * 10);
    let brk = sys_sbrk(0);
    println!("10 page allocated,  break point = {:x}", brk);
    sys_sbrk(PAGE_SIZE as i32 * -11);
    let brk = sys_sbrk(0);
    println!("11 page DEALLOCATED,  break point = {:x}", brk);
    println!("try DEALLOCATED more one page, should be failed.");
    let ret = sys_sbrk(PAGE_SIZE as i32 * -1);
    if ret != -1 {
        println!("Test sys_sbrk failed!");
        return -1;
    }
    println!("Test sys_sbrk almost OK!");
    println!("now write to deallocated page, should cause page fault.");
    for pos in 0..PAGE_SIZE {
        new_page[pos] = 2;
    }
    0
}
