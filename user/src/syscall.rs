// 系统调用号
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;

// 调用系统调用：写文件到缓冲区
// - fd：待写入文件的文件描述符；
// - buf：内存中缓冲区的起始地址；
// - len：内存中缓冲区的长度。
// - 返回值：成功写入的长度。
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    // buffer.as_ptr()表示buffer的指针
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

// 调用系统调用：退出应用程序
// - exit_code：程序的退出码。
pub fn sys_exit(exit_code: i32) -> isize {
    syscall(SYSCALL_EXIT, [exit_code as usize, 0, 0])
}

// 调用系统调用：程序主动让出CPU，调度到其他程序
pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}

// 封装系统调用的调用
// 内核提供的系统调用，是汇编级别的二进制接口，所以要手写汇编。
fn syscall(id: usize, args: [usize; 3]) -> isize {
    use core::arch::asm;
    let mut ret: isize;
    unsafe {
        // 宏asm!用于内联汇编，即在Rust代码中嵌入汇编代码。
        asm!(
            // 使用RISC-V的ecall指令触发系统调用。
            // ecall指令将触发异常，从User级别切换到Supervisor级别，才能执行内核提供的系统调用。
            "ecall",
            // 系统调用
            // - 执行时，寄存器x17（又叫a7）存放系统调用号。
            // -       寄存器x10/x11/x12（又叫a0/a1/a2）存放参数。
            // - 返回时，寄存器x10（又叫a0）存放返回值（由这里的ret接收）。
            inlateout("x10") args[0] => ret,
            in("x11") args[1],
            in("x12") args[2],
            // 系统调用执行时，寄存器x17（又叫a7）存放系统调用号。
            in("x17") id
        );
    }
    ret
}
