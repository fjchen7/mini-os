use crate::config::VIRT_PLIC;
use crate::drivers::block::BLOCK_DEVICE;
use crate::drivers::chardev::{CharDevice, UART};
use crate::drivers::plic::{IntrTargetPriority, PLIC};

// 初始化PLIC和sie寄存器，使其能够响应外设中断
pub fn device_init() {
    use riscv::register::sie;
    let mut plic = unsafe { PLIC::new(VIRT_PLIC) };
    let hart_id: usize = 0;
    let supervisor = IntrTargetPriority::Supervisor;
    let machine = IntrTargetPriority::Machine;
    // 设置M和S特权级下，PLIC要响应的外设中断阈值
    plic.set_threshold(hart_id, supervisor, 0);
    plic.set_threshold(hart_id, machine, 1);
    // S特权级下，允许PLIC传递键盘/鼠标/块设备/串口外设中断
    // irq（Interrupt Request）编号: 5 键盘、 6 鼠标、8 块设备、10 uart（串口）
    for intr_src_id in [5usize, 6, 8, 10] {
        plic.enable(hart_id, supervisor, intr_src_id);
        plic.set_priority(intr_src_id, 1);
    }
    // 将sie寄存器设为1，开启S-Mode下的外部中断
    unsafe {
        sie::set_sext();
    }
}

// 处理外设中断
pub fn irq_handler() {
    let mut plic = unsafe { PLIC::new(VIRT_PLIC) };
    // 读取PLIC的Claim寄存器，获得接收到的外设中断号
    let intr_src_id = plic.claim(0, IntrTargetPriority::Supervisor);
    match intr_src_id {
        8 => BLOCK_DEVICE.handle_irq(),
        10 => UART.handle_irq(),
        _ => panic!("unsupported IRQ {}", intr_src_id),
    }
    // 通知PLIC中断已处理完毕
    plic.complete(0, IntrTargetPriority::Supervisor, intr_src_id);
}
