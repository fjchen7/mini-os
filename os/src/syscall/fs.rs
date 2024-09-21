//! 文件系统相关的系统调用
use alloc::sync::Arc;
use core::any::Any;

use crate::config::PAGE_SIZE;
use crate::fs::{make_pipe, open_file, OSInode, OpenFlags};
use crate::mm::{
    translated_byte_buffer, translated_refmut, translated_str, FileMapping, UserBuffer,
};
use crate::task::{current_process, current_user_token};

// 将buf中长度为len的字节，写入到文件fd中
// 返回值：成功写入的字节数。如果出错则返回-1。
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

// 从文件fd中读取长度为len的字节，写入到buf中
// 返回值：成功读取的字节数。如果出错则返回-1。
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        drop(inner);
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

// 打开一个文件
// - path: 文件路径
// - flags: 打开文件的标志
// 返回值：返回打开文件的文件描述符。如果出错则返回 -1。
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    let process = current_process();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

// 将文件映射到内存中。映射成功后，可以通过内存地址直接访问文件的内容。
// 被映射的文件区域为[offset, offset + len)。
pub fn sys_mmap(fd: usize, len: usize, offset: usize) -> isize {
    if len == 0 {
        // invalid length
        return -1;
    }
    if (offset & (PAGE_SIZE - 1)) != 0 {
        // offset must be page size aligned
        return -1;
    }

    let process = current_process();
    let mut tcb = process.inner_exclusive_access();
    if fd >= tcb.fd_table.len() {
        return -1;
    }
    if tcb.fd_table[fd].is_none() {
        return -1;
    }

    let fp = tcb.fd_table[fd].as_ref().unwrap();
    let any: &dyn Any = fp;
    let opt_inode = any.downcast_ref::<OSInode>();
    // let opt_inode = fp.as_any().downcast_ref::<OSInode>();
    if opt_inode.is_none() {
        // must be a regular file
        return -1;
    }

    let inode = opt_inode.unwrap();
    let perm = inode.map_permission();
    let file = inode.clone_inner_inode();
    if offset >= file.size() as usize {
        // file offset exceeds size limit
        return -1;
    }

    let start = tcb.mmap_va_allocator.alloc(len);
    // 现在只记录映射关系，不实际分配物理页。访问时再分配。
    if let Some(m) = tcb.find_file_mapping_mut(&file) {
        m.push(start, len, offset, perm);
    } else {
        let mut m = FileMapping::new_empty(file);
        m.push(start, len, offset, perm);
        tcb.file_mappings.push(m);
    }
    start.0 as isize
}

// 为当前进程创建一个管道。
// - pipe：应用地址空间中，长度为 2 的 usize 数组的起始地址。该方法需要将所创建的读和写管道的文件描述符，写入到该数组中。
// - 返回值：0成功，-1错误（如传入的地址不合法）。
pub fn sys_pipe(pipe: *mut usize) -> isize {
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}

// 将当前进程的已打开的文件，复制并分配到一个新的文件描述符中。
// 实质是分配一个新的文件描述符，指向同一个文件对象。
// - fd：进程的已经打开文件的描述符。
// - 返回值：成功则返回新的文件描述符，失败则返回 -1（比如传入的 fd 不合法）。
pub fn sys_dup(fd: usize) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}
