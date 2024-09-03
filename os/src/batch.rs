use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use core::arch::asm;
use lazy_static::*;

const MAX_APP_NUM: usize = 16;
const APP_BASE_ADDRESS: usize = 0x80400000;
const APP_SIZE_LIMIT: usize = 0x20000;

lazy_static! {
    // 我们想声明一个全局且可变的变量，用于管理程序
    // lazy_static满足全局，UpSafeCell满足可变（内部可变性）
    static ref APP_MANAGER: UPSafeCell<AppManager> = unsafe { UPSafeCell::new({
        // 找到符号_num_app的位置
        extern "C" { fn _num_app(); }
        let num_app_ptr = _num_app as usize as *const usize;
        let num_app = num_app_ptr.read_volatile();
        let mut app_start: [usize; MAX_APP_NUM + 1] = [0; MAX_APP_NUM + 1];
        let app_start_raw: &[usize] =  core::slice::from_raw_parts(
            num_app_ptr.add(1), num_app + 1
        );
        app_start[..=num_app].copy_from_slice(app_start_raw);
        AppManager {
            num_app,
            current_app: 0,
            app_start,
        }
    })};
}

struct AppManager {
    // 已加载到内存的程序数量（这里加载进.data段）
    num_app: usize,
    // 当前运行的程序
    current_app: usize,
    // 每个程序在内存中的起始地址
    app_start: [usize; MAX_APP_NUM + 1],
}

impl AppManager {
    pub fn print_app_info(&self) {
        println_kernel!("num_app = {}", self.num_app);
        for i in 0..self.num_app {
            println_kernel!(
                "app_{} [{:#x}, {:#x})",
                i,
                self.app_start[i],
                self.app_start[i + 1]
            );
        }
    }

    unsafe fn load_app(&self, app_id: usize) {
        if app_id >= self.num_app {
            println!("All applications completed!");
            shutdown(false);
        }
        println_kernel!("Loading app_{}", app_id);
        // 清空上一个应用程序加载的内存区域
        // 起始位置为Linux约定的0x80400000，最大长度是我们定的
        core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, APP_SIZE_LIMIT).fill(0);
        // 将应用程序的代码加载到内存中
        let app_src = core::slice::from_raw_parts(
            self.app_start[app_id] as *const u8,
            self.app_start[app_id + 1] - self.app_start[app_id],
        );
        let app_dst = core::slice::from_raw_parts_mut(APP_BASE_ADDRESS as *mut u8, app_src.len());
        app_dst.copy_from_slice(app_src);
        // CPU有指令缓存（i-cache）。
        // 一般情况下，.text段不会被修改，因此不会出现缓存与内存中的指令不一致的情况。
        // 但在这里，我们覆盖了.text段，所以需要内存屏障指令fence.i，来刷新指令缓存。
        // 参见：riscv非特权规范第3章，“Zifencei”扩展。
        asm!("fence.i");
    }

    pub fn get_current_app(&self) -> usize {
        self.current_app
    }

    pub fn move_to_next_app(&mut self) {
        self.current_app += 1;
    }
}

// 初始化批处理的子系统
pub fn init() {
    print_app_info();
}

// 打印应用程序信息
pub fn print_app_info() {
    APP_MANAGER.exclusive_access().print_app_info();
}

// 运行下一个应用程序
pub fn run_next_app() -> ! {
    let mut app_manager = APP_MANAGER.exclusive_access();
    let current_app = app_manager.get_current_app();
    unsafe {
        app_manager.load_app(current_app);
    }
    app_manager.move_to_next_app();
    drop(app_manager); // 使用完RefCell的可变借用后，就马上手动释放。这是一个好习惯！
    extern "C" {
        fn __restore(cx_addr: usize);
    }
    unsafe {
        let trap_ctx = TrapContext::app_init_context(APP_BASE_ADDRESS, USER_STACK.get_sp());
        let ctx_ptr = KERNEL_STACK.push_context(trap_ctx);
        __restore(ctx_ptr as *const _ as usize);
    }
    // __restore方法最后执行sret指令。该指令将spec寄存器的值写入到PC寄存器中，从而跳转回用户态。
    // 在这里，pc的值会变成APP_BASE_ADDRESS，也就是回到应用程序的入口地址。
    // 因此，永远不会执行到此处。
    panic!("Unreachable in batch::run_current_app!");
}

// 用户栈和内核栈的大小都是8KB
const USER_STACK_SIZE: usize = 4096 * 2;
const KERNEL_STACK_SIZE: usize = 4096 * 2;

#[repr(align(4096))]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: KernelStack = KernelStack {
    data: [0; KERNEL_STACK_SIZE],
};
static USER_STACK: UserStack = UserStack {
    data: [0; USER_STACK_SIZE],
};

impl KernelStack {
    // 获取内核栈的栈顶指针
    // self.data.as_ptr()是栈底，加上栈的大小就是栈顶
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    // 将TrapContext压入内核栈
    pub fn push_context(&self, cx: TrapContext) -> &'static mut TrapContext {
        let cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *cx_ptr = cx;
        }
        unsafe { cx_ptr.as_mut().unwrap() }
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}
