//! 块设备的抽象接口

use core::any::Any;

// 数据块的读写接口。在这里，一个块只包含一个扇区（512字节）。
// 存储设备的驱动，需要实现这个接口
pub trait BlockDevice: Send + Sync + Any {
    // 将编号为block_id的块中的数据，读到buf中
    fn read_block(&self, block_id: usize, buf: &mut [u8]);
    // 将buf中的数据，写入编号为block_id的块中
    fn write_block(&self, block_id: usize, buf: &[u8]);
    // 中断处理函数
    fn handle_irq(&self);
}
