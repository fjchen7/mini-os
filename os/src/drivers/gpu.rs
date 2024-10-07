use crate::drivers::bus::VirtioHal;
use crate::sync::UPIntrFreeCell;
use alloc::{sync::Arc, vec::Vec};
use core::any::Any;
use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;
use virtio_drivers::{VirtIOGpu, VirtIOHeader};

pub trait GpuDevice: Send + Sync + Any {
    #[allow(dead_code)]
    fn update_cursor(&self);
    fn get_framebuffer(&self) -> &mut [u8];
    fn flush(&self);
}

lazy_static::lazy_static!(
    // 用于访问GPU设备的全局变量
    pub static ref GPU_DEVICE: Arc<dyn GpuDevice> = Arc::new(VirtIOGpuWrapper::new());
);

struct VirtIOGpuWrapper {
    gpu: UPIntrFreeCell<VirtIOGpu<'static, VirtioHal>>,
    // 显存的缓冲区
    fb: &'static [u8],
}
static BMP_DATA: &[u8] = include_bytes!("../assert/mouse.bmp");
impl VirtIOGpuWrapper {
    // 初始化virtio-gpu设备
    pub fn new() -> Self {
        unsafe {
            let mut virtio =
                VirtIOGpu::<VirtioHal>::new(&mut *(crate::config::VIRTIO7 as *mut VirtIOHeader))
                    .unwrap();
            // 初始化显存
            let fbuffer = virtio.setup_framebuffer().unwrap();
            let len = fbuffer.len();
            let ptr = fbuffer.as_mut_ptr();
            let fb = core::slice::from_raw_parts_mut(ptr, len);
            // 初始化光标图像
            let bmp = Bmp::<Rgb888>::from_slice(BMP_DATA).unwrap();
            let raw = bmp.as_raw();
            let mut b = Vec::new();
            for i in raw.image_data().chunks(3) {
                let mut v = i.to_vec();
                b.append(&mut v);
                if i == [255, 255, 255] {
                    b.push(0x0)
                } else {
                    b.push(0xff)
                }
            }
            // 设置光标图像
            virtio.setup_cursor(b.as_slice(), 50, 50, 50, 50).unwrap();
            Self {
                gpu: UPIntrFreeCell::new(virtio),
                fb,
            }
        }
    }
}

impl GpuDevice for VirtIOGpuWrapper {
    // 通知virtio-gpu设备，刷新显示内容
    fn flush(&self) {
        self.gpu.exclusive_access().flush().unwrap();
    }

    // 得到显存的缓冲区地址（内核空间）
    fn get_framebuffer(&self) -> &mut [u8] {
        unsafe {
            let ptr = self.fb.as_ptr() as *const _ as *mut u8;
            core::slice::from_raw_parts_mut(ptr, self.fb.len())
        }
    }

    fn update_cursor(&self) {}
}
