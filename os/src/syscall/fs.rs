//! 文件系统相关的系统调用

use crate::{mm::translated_byte_buffer, task::TASK_MANAGER};
const FD_STDOUT: usize = 1;

// 将长度为`len`的buf写入文件描述符为`fd`的文件
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    match fd {
        FD_STDOUT => {
            let buffers = translated_byte_buffer(TASK_MANAGER.get_current_token(), buf, len);
            for buffer in buffers {
                print!("{}", core::str::from_utf8(buffer).unwrap());
            }
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}
