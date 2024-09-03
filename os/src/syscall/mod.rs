const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;

mod fs;
mod process;

use fs::*;
use process::*;

// 实现系统调用
// 程序调用ecall指令时，将触发系统调用（UserEnvCall类型的异常），并由trap_handler方法处理，最后进入本方法。
// 这里不关心哪些寄存器存放参数和返回值。这由trap_handler方法确定。
pub fn syscall(syscall_id: usize, args: [usize; 3]) -> isize {
    match syscall_id {
        SYSCALL_WRITE => sys_write(args[0], args[1] as *const u8, args[2]),
        SYSCALL_EXIT => sys_exit(args[0] as i32),
        _ => panic!("Unsupported syscall_id: {}", syscall_id),
    }
}
