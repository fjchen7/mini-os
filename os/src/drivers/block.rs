use super::bus::VirtioHal;
use crate::sync::Condvar;
use crate::sync::UPIntrFreeCell;
use crate::task::schedule;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use easy_fs::BlockDevice;
use lazy_static::*;
use virtio_drivers::{BlkResp, RespStatus, VirtIOBlk, VirtIOHeader};

lazy_static! {
    // 用于访问块设备的全局变量
    pub static ref BLOCK_DEVICE: Arc<dyn BlockDevice> = Arc::new(VirtIOBlock::new());
    // 该字段表示，是否以非阻塞方式（中断）方式访问块设备
    pub static ref DEV_NON_BLOCKING_ACCESS: UPIntrFreeCell<bool> = unsafe {
        UPIntrFreeCell::new(false)
    };
}

#[allow(unused)]
pub fn block_device_test() {
    let block_device = BLOCK_DEVICE.clone();
    let mut write_buffer = [0u8; 512];
    let mut read_buffer = [0u8; 512];
    for i in 0..512 {
        for byte in write_buffer.iter_mut() {
            *byte = i as u8;
        }
        block_device.write_block(i as usize, &write_buffer);
        block_device.read_block(i as usize, &mut read_buffer);
        assert_eq!(write_buffer, read_buffer);
    }
    println!("block device test passed!");
}

struct VirtIOBlock {
    virtio_blk: UPIntrFreeCell<VirtIOBlk<'static, VirtioHal>>,
    // 在等待I/O操作完成前，会挂起进程。等待I/O操作完成时，通过该条件变量唤醒进程
    // 此处是一个条件变量队列，每个元素都对应着virtqueue的一个条目。这表示每个I/O请求，都会绑定一个条件变量
    condvars: BTreeMap<u16, Condvar>,
}

impl VirtIOBlock {
    pub fn new() -> Self {
        let virtio_blk = unsafe {
            UPIntrFreeCell::new(
                // 以MMIO方式访问VirtIO块设备的寄存器，VirtIOHeader表示该组寄存器
                VirtIOBlk::<VirtioHal>::new(&mut *(crate::config::VIRTIO0 as *mut VirtIOHeader))
                    .unwrap(),
            )
        };
        let mut condvars = BTreeMap::new();
        let channels = virtio_blk.exclusive_access().virt_queue_size();
        for i in 0..channels {
            let condvar = Condvar::new();
            condvars.insert(i, condvar);
        }
        Self {
            virtio_blk,
            condvars,
        }
    }
}

impl BlockDevice for VirtIOBlock {
    fn read_block(&self, block_id: usize, buf: &mut [u8]) {
        let nb = *DEV_NON_BLOCKING_ACCESS.exclusive_access();
        if nb {
            // 以非阻塞方式（中断）访问块设备
            let mut resp = BlkResp::default();
            let task_cx_ptr = self.virtio_blk.exclusive_session(|blk| {
                let token = unsafe { blk.read_block_nb(block_id, buf, &mut resp).unwrap() };
                self.condvars.get(&token).unwrap().wait_no_scheduled()
            });
            schedule(task_cx_ptr);
            assert_eq!(
                resp.status(),
                RespStatus::Ok,
                "Error when reading VirtIOBlk"
            );
        } else {
            // 以阻塞方式（轮询）访问块设备
            self.virtio_blk
                .exclusive_access()
                .read_block(block_id, buf)
                .expect("Error when reading VirtIOBlk");
        }
    }

    fn write_block(&self, block_id: usize, buf: &[u8]) {
        let nb = *DEV_NON_BLOCKING_ACCESS.exclusive_access();
        if nb {
            // 以非阻塞方式（中断）访问块设备
            let mut resp = BlkResp::default();
            let task_cx_ptr = self.virtio_blk.exclusive_session(|blk| {
                let token = unsafe { blk.write_block_nb(block_id, buf, &mut resp).unwrap() };
                self.condvars.get(&token).unwrap().wait_no_scheduled()
            });
            schedule(task_cx_ptr);
            assert_eq!(
                resp.status(),
                RespStatus::Ok,
                "Error when writing VirtIOBlk"
            );
        } else {
            // 以阻塞方式（轮询）访问块设备
            self.virtio_blk
                .exclusive_access()
                .write_block(block_id, buf)
                .expect("Error when writing VirtIOBlk");
        }
    }

    fn handle_irq(&self) {
        self.virtio_blk.exclusive_session(|blk| {
            while let Ok(token) = blk.pop_used() {
                self.condvars.get(&token).unwrap().signal();
            }
        });
    }
}
