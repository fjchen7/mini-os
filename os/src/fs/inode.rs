//! 将文件系统的inode包装成内核的inode，即OSInode。该类型供进程使用，表示一个被打开的文件。

use super::File;
use crate::mm::UserBuffer;
use crate::sync::UPIntrFreeCell;
use crate::{drivers::BLOCK_DEVICE, mm::MapPermission};
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;

// OSInode表示一个被打开的文件。多个进程可打开同一个文件。
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPIntrFreeCell<OSInodeInner>,
}

pub struct OSInodeInner {
    // 进程读写文件的偏移量
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        Self {
            readable,
            writable,
            inner: unsafe { UPIntrFreeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }

    // 读出所有的数据
    pub fn read_all(&self) -> Vec<u8> {
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }

    pub fn clone_inner_inode(&self) -> Arc<Inode> {
        self.inner.exclusive_access().inode.clone()
    }

    pub fn map_permission(&self) -> MapPermission {
        if self.readable && self.writable {
            MapPermission::R | MapPermission::W
        } else if self.readable {
            MapPermission::R
        } else if self.writable {
            MapPermission::W
        } else {
            MapPermission::empty()
        }
    }
}

lazy_static! {
    // 根目录的inode
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

// 列出根目录下的所有应用程序
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    // 打开文件的标志位
    pub struct OpenFlags: u32 {
        const RDONLY = 0;       // 只读
        const WRONLY = 1 << 0;  // 只写
        const RDWR = 1 << 1;    // 读写
        const CREATE = 1 << 9;  // 创建。如果文件存在，则截断文件
        const TRUNC = 1 << 10;  // 截断，即删除文件中原有的内容
    }
}

impl OpenFlags {
    /// 返回 (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

// 打开根目录下的一个文件
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // 如果文件存在，则清空文件
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

impl File for OSInode {
    fn readable(&self) -> bool {
        self.readable
    }
    fn writable(&self) -> bool {
        self.writable
    }
    fn read(&self, mut buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
}
