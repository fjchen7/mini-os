mod ns16550a;

use alloc::sync::Arc;
use lazy_static::*;
pub use ns16550a::NS16550a;

use crate::config::VIRT_UART;

pub type CharDeviceImpl = crate::drivers::chardev::NS16550a<VIRT_UART>;

pub trait CharDevice {
    fn init(&self);
    fn read(&self) -> u8;
    fn write(&self, ch: u8);
    fn handle_irq(&self);
}

lazy_static! {
    pub static ref UART: Arc<CharDeviceImpl> = Arc::new(CharDeviceImpl::new());
}
