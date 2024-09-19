use riscv::register::sstatus::{self, Sstatus, SPP};

#[repr(C)]
#[derive(Debug, Clone, Copy)]
// Trap上下文
// 进入/退出Trap时，要恢复的寄存器只有：通用寄存器x[32]、sstatus、sepc
// 剩余的kernel_satp、kernel_sp、trap_handler，在切换地址空间时使用
pub struct TrapContext {
    // 通用寄存器
    pub x: [usize; 32],
    // CSR寄存器sstatus，记录Trap发生之前，CPU处于哪个特权级（S/U）
    pub sstatus: Sstatus,
    // CSR寄存器sepc，记录Trap发生之前执行的最后一条指令地址
    // Trap处理完，执行sret回到User模式后，spec的值会被复制到pc寄存器，CPU从这里继续执行
    // 如果是系统调用，那sepc就指向ecall指令（从U切换S模式）的地址
    pub sepc: usize,

    // 记录内核地址空间所对应的satp寄存器的值。satp寄存器设置分页模式和根页表的物理地址。
    pub kernel_satp: usize,
    // 在内核地址空间中，属于该程序的内核栈的栈顶地址。
    pub kernel_sp: usize,
    // 处理Trapt的方法trap_handler的地址
    pub trap_handler: usize,
}

impl TrapContext {
    // 设置栈指针，寄存器x2存放的是栈顶指针sp
    pub fn set_sp(&mut self, sp: usize) {
        self.x[2] = sp;
    }

    // 初始化程序的TrapContext
    pub fn app_init_context(
        entry: usize,
        sp: usize,
        kernel_satp: usize,
        kernel_sp: usize,
        trap_handler: usize,
    ) -> Self {
        // 设置CSR寄存器sstatus，记录Trap发生之前，CPU处于哪个特权级（S/U）
        // 由于是应用程序，所以肯定处于U模式
        let mut sstatus = sstatus::read();
        sstatus.set_spp(SPP::User);
        let mut cx = Self {
            x: [0; 32],
            sstatus,
            sepc: entry,  // 程序的入口地址（在.text段上）
            kernel_satp,  // 内核地址空间对应的satp寄存器的值
            kernel_sp,    // 内核地址空间中，属于该程序的内核栈的栈顶指针
            trap_handler, // trap_handler方法的地址
        };
        // 设置程序的用户栈的栈顶指针
        cx.set_sp(sp);
        cx
    }
}
