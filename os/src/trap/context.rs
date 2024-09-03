use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
// 表示处理Trap时，要保存的上下文信息
pub struct TrapContext {
    // 通用寄存器
    pub x: [usize; 32],
    // CSR寄存器sstatus，记录Trap发生之前，CPU处于哪个特权级（S/U）
    pub sstatus: Sstatus,
    // CSR寄存器sepc，记录Trap发生之前执行的最后一条指令地址
    // Trap处理完，执行sret回到User模式后，spec的值会被复制到pc寄存器，CPU从这里继续执行
    // 如果是系统调用，那sepc就指向ecall指令（从U切换S模式）的地址
    pub sepc: usize,
}

impl TrapContext {
    // 设置栈指针，寄存器x2存放的是栈顶指针sp
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    // 初始化应用程序的TrapContext
    pub fn app_init_context(entry: usize, sp: usize) -> Self {
        // 设置CSR寄存器sstatus，记录Trap发生之前，CPU处于哪个特权级（S/U）
        // 由于是应用程序，所以肯定处于U模式
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry, // 应用程序的入口地址（在.text段上）
        };
        // 设置用户程序的栈顶指针
        cx.set_sp(sp);
        cx
    }
}
