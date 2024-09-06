//! 实现动态地分配堆内存

use crate::config::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;

// 指定全局内存分配器
// LockedHeap实现了std::alloc::GlobalAlloc
#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

// 初始化堆空间：提供一块内存空间（HEAP_SPACE），作为「初始堆」。
// HEAP_SPACE是一个未初始化的静态变量，它位于.bss段中。因此这个堆也位于.bss段。
pub fn init_heap() {
    unsafe {
        // LockedHeap是被Mutex<T>包装的类型
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

#[alloc_error_handler]
// 处理内存分配错误
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

#[allow(unused)]
// 该函数用于测试，可放进main里。
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    // Box的创建使用了我们的全局内存分配器
    // 分配好后，检查它是否在bss段内（我们的堆空间在bss段）
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    // 同样检查对Vec是否生效
    let mut v: Vec<usize> = Vec::new();
    (0..500).for_each(|i| v.push(i));
    for (i, val) in v.iter().take(500).enumerate() {
        assert_eq!(*val, i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}
