use core::{
    cell::{RefCell, RefMut, UnsafeCell},
    ops::{Deref, DerefMut},
};
use lazy_static::*;
use riscv::register::sstatus;

/*
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
*/


// UPIntrFreeCell的功能与UpSafeCell一样，也是为了提供一个可变的全局变量。
// 但它多了一个功能：拿到可变引用时，自动屏蔽中断；drop时，自动打开中断。
// 这是为了确保能对内部数据进行独占访问。
pub struct UPIntrFreeCell<T> {
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPIntrFreeCell<T> {}

pub struct UPIntrRefMut<'a, T>(Option<RefMut<'a, T>>);

impl<T> UPIntrFreeCell<T> {
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    // 如果数据已经被借用，会panic
    pub fn exclusive_access(&self) -> UPIntrRefMut<'_, T> {
        INTR_MASKING_INFO.get_mut().enter();
        UPIntrRefMut(Some(self.inner.borrow_mut()))
    }

    pub fn exclusive_session<F, V>(&self, f: F) -> V
    where
        F: FnOnce(&mut T) -> V,
    {
        let mut inner = self.exclusive_access();
        f(inner.deref_mut())
    }
}

impl<'a, T> Drop for UPIntrRefMut<'a, T> {
    fn drop(&mut self) {
        self.0 = None;
        INTR_MASKING_INFO.get_mut().exit();
    }
}

impl<'a, T> Deref for UPIntrRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap().deref()
    }
}
impl<'a, T> DerefMut for UPIntrRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap().deref_mut()
    }
}

struct UPSafeCellRaw<T> {
    inner: UnsafeCell<T>,
}

unsafe impl<T> Sync for UPSafeCellRaw<T> {}

impl<T> UPSafeCellRaw<T> {
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut (*self.inner.get()) }
    }
}

struct IntrMaskingInfo {
    nested_level: usize,
    sie_before_masking: bool,
}

lazy_static! {
    static ref INTR_MASKING_INFO: UPSafeCellRaw<IntrMaskingInfo> =
        unsafe { UPSafeCellRaw::new(IntrMaskingInfo::new()) };
}

impl IntrMaskingInfo {
    pub fn new() -> Self {
        Self {
            nested_level: 0,
            sie_before_masking: false,
        }
    }

    // 屏蔽中断
    pub fn enter(&mut self) {
        let sie = sstatus::read().sie();
        unsafe {
            sstatus::clear_sie();
        }
        if self.nested_level == 0 {
            self.sie_before_masking = sie;
        }
        self.nested_level += 1;
    }

    // 打开中断
    pub fn exit(&mut self) {
        self.nested_level -= 1;
        if self.nested_level == 0 && self.sie_before_masking {
            unsafe {
                sstatus::set_sie();
            }
        }
    }
}
