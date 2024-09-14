use crate::mm::UserBuffer;

// 内核的文件抽象
pub trait File : Send + Sync {
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
}
