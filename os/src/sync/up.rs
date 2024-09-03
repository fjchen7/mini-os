use core::cell::{RefCell, RefMut};

// 该类型包装RefCell，并实现了Sync特征，以便我们能将该类型初始化成全局静态变量。
// UP表示单核（uniprocessor），即该类型只被设计在单核环境下使用。
pub struct UPSafeCell<T> {
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    // 用户需要保证，内部结构只在单核环境下使用。
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    // 该函数返回一个可变引用，允许用户修改内部数据。
    // 但使用方法遵循RefCell的规则，同一时刻只能有一个可变引用。
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
