// 系统调用号
const SYSCALL_READ: usize = 63;
const SYSCALL_WRITE: usize = 64;
const SYSCALL_EXIT: usize = 93;
const SYSCALL_YIELD: usize = 124;
const SYSCALL_GET_TIME: usize = 169;
const SYSCALL_SBRK: usize = 214;
const SYSCALL_GETPID: usize = 172;
const SYSCALL_FORK: usize = 220;
const SYSCALL_EXEC: usize = 221;
const SYSCALL_WAITPID: usize = 260;
const SYSCALL_OPEN: usize = 56;
const SYSCALL_CLOSE: usize = 57;

pub fn sys_open(path: &str, flags: u32) -> isize {
    syscall(SYSCALL_OPEN, [path.as_ptr() as usize, flags as usize, 0])
}

pub fn sys_close(fd: usize) -> isize {
    syscall(SYSCALL_CLOSE, [fd, 0, 0])
}

// 读取文件到内存缓冲区
// - fd：待读取文件的文件描述符；
// - buf：缓冲区的起始地址。读出的最大长度为buf.len()。
// - 返回值：实际读取的字节数。-1表示错误。
pub fn sys_read(fd: usize, buffer: &mut [u8]) -> isize {
    syscall(
        SYSCALL_READ,
        [fd, buffer.as_mut_ptr() as usize, buffer.len()],
    )
}

// 写文件到缓冲区
// - fd：待写入文件的文件描述符；
// - buf：内存中缓冲区的起始地址；
// - len：内存中缓冲区的长度。
// - 返回值：成功写入的长度。
pub fn sys_write(fd: usize, buffer: &[u8]) -> isize {
    // buffer.as_ptr()表示buffer的指针
    syscall(SYSCALL_WRITE, [fd, buffer.as_ptr() as usize, buffer.len()])
}

// 退出应用程序
pub fn sys_exit(exit_code: i32) -> ! {
    syscall(SYSCALL_EXIT, [exit_code as usize, 0, 0]);
    unreachable!("sys_exit never returns!")
}

// 程序主动让出CPU，调度到其他程序
pub fn sys_yield() -> isize {
    syscall(SYSCALL_YIELD, [0, 0, 0])
}

// 增加或减少堆的大小。返回旧的堆顶地址。
pub fn sys_sbrk(size: i32) -> isize {
    syscall(SYSCALL_SBRK, [size as usize, 0, 0])
}

// 获取CPU时间（ms）
pub fn sys_get_time() -> isize {
    syscall(SYSCALL_GET_TIME, [0, 0, 0])
}

pub fn sys_getpid() -> isize {
    syscall(SYSCALL_GETPID, [0, 0, 0])
}

// 复制出一个子进程，返回子进程的PID
pub fn sys_fork() -> isize {
    syscall(SYSCALL_FORK, [0, 0, 0])
}

// 将ELF可执行文件加载到当前进程的地址空间，并开始执行。
// - path：ELF文件的路径。
// - 返回值：执行成功则不返回，失败则返回-1。
pub fn sys_exec(path: &str) -> isize {
    syscall(SYSCALL_EXEC, [path.as_ptr() as usize, 0, 0])
}

// 找到当前进程的僵尸子进程，回收全部资源
// - pid：要找的子进程PID，-1表示等待任意子进程；
// - exit_code：保存子进程的返回值的地址，为0表示不保存。
// - 返回值：
//   - -1：找不到对应的子进程；
//   - -2：等待的子进程均未退出；
//   - 其他：结束的子进程的PID
pub fn sys_waitpid(pid: isize, exit_code: *mut i32) -> isize {
    syscall(SYSCALL_WAITPID, [pid as usize, exit_code as usize, 0])
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
