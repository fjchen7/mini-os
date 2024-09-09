//! 将用户应用程序加载到内存中

use crate::config::*;
use crate::trap::TrapContext;
use core::arch::asm;

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

static KERNEL_STACK: [KernelStack; MAX_APP_NUM] = [KernelStack {
    data: [0; KERNEL_STACK_SIZE],
}; MAX_APP_NUM];

static USER_STACK: [UserStack; MAX_APP_NUM] = [UserStack {
    data: [0; USER_STACK_SIZE],
}; MAX_APP_NUM];

impl KernelStack {
    // 获取内核栈的栈顶指针
    fn get_sp(&self) -> usize {
        // self.data.as_ptr()是栈底，加上栈的大小就是栈顶
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    // 将TrapContext压入内核栈
    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}

// 返回第i个应用程序的基地址（Base Address），即在.text段中的起始地址
// 该地址与../user/build.py生成的一致
fn get_base_i(app_id: usize) -> usize {
    APP_BASE_ADDRESS + app_id * APP_SIZE_LIMIT
}

// 返回应用程序的总数
pub fn get_num_app() -> usize {
    extern "C" {
        fn _num_app();
    }
    unsafe { (_num_app as usize as *const usize).read_volatile() }
}

// 将所有的程序都加载到内存中（准确地说，应该是从.data段复制到.text段）
// 第i个应用程序，会被复制到内存区间 [APP_BASE_ADDRESS + i * APP_SIZE_LIMIT, APP_BASE_ADDRESS + (i+1) * APP_SIZE_LIMIT).
pub fn load_apps() {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    // 应用程序在.data段的起始地址
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    for i in 0..num_app {
        let base_i = get_base_i(i);
        // 清空应用程序的内存区域
        (base_i..base_i + APP_SIZE_LIMIT)
            .for_each(|addr| unsafe { (addr as *mut u8).write_volatile(0) });
        // 将应用程序的二进制，从.data段复制到.text段中
        let src = unsafe {
            core::slice::from_raw_parts(app_start[i] as *const u8, app_start[i + 1] - app_start[i])
        };
        let dst = unsafe { core::slice::from_raw_parts_mut(base_i as *mut u8, src.len()) };
        dst.copy_from_slice(src);
    }
    unsafe {
        // CPU有指令缓存（i-cache）。
        // 一般情况下，.text段不会被修改，因此不会出现缓存与内存中的指令不一致的情况。
        // 但在这里，我们覆盖了.text段，所以需要内存屏障指令fence.i，来刷新指令缓存。
        // 参见：riscv非特权规范第3章，“Zifencei”扩展。
        asm!("fence.i");
    }
}

// 返回应用程序的ELF可执行文件的二进制数据
// 根据build.rs生成的link_app.S设定的符号，来拿到这个数据
pub fn get_app_data(app_id: usize) -> &'static [u8] {
    extern "C" {
        fn _num_app();
    }
    let num_app_ptr = _num_app as usize as *const usize;
    let num_app = get_num_app();
    let app_start = unsafe { core::slice::from_raw_parts(num_app_ptr.add(1), num_app + 1) };
    assert!(app_id < num_app);
    unsafe {
        core::slice::from_raw_parts(
            app_start[app_id] as *const u8,
            app_start[app_id + 1] - app_start[app_id],
        )
    }
}

// 初始化第i个程序的Trap上下文，这包括
// - 将入口设置为程序的.text段起始地址（spec）
// - 设置程序栈的栈顶指针（sp）
// 返回该上下文会被保存到的内核栈中的地址
pub fn init_app_cx(app_id: usize) -> usize {
    let base_i = get_base_i(app_id);
    // 当前的代码没有用到程序栈，因此sp的值可以是任意的，只要不覆盖其他数据，或超出内存范围即可。
    let sp = USER_STACK[app_id].get_sp();
    let trap_ctx = TrapContext::app_init_context(base_i, sp);
    KERNEL_STACK[app_id].push_context(trap_ctx)
}
