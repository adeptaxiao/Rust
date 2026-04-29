//! Власна реалізація Arc через атомарний лічильник посилань.
use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ArcInner<T> {
    ref_count: AtomicUsize,
    data: T,
}

/// Потокобезпечний розумний вказівник з підрахунком посилань.
pub struct MyArc<T> {
    ptr: NonNull<ArcInner<T>>,
}

unsafe impl<T: Send + Sync> Send for MyArc<T> {}
unsafe impl<T: Send + Sync> Sync for MyArc<T> {}

impl<T> MyArc<T> {
    /// Створює новий `MyArc` з переданим значенням.
    pub fn new(data: T) -> Self {
        let inner = Box::new(ArcInner { ref_count: AtomicUsize::new(1), data });
        Self { ptr: NonNull::new(Box::into_raw(inner)).unwrap() }
    }

    fn inner(&self) -> &ArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> Clone for MyArc<T> {
    fn clone(&self) -> Self {
        self.inner().ref_count.fetch_add(1, Ordering::Relaxed);
        Self { ptr: self.ptr }
    }
}

impl<T> Deref for MyArc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner().data
    }
}

impl<T> Drop for MyArc<T> {
    fn drop(&mut self) {
        if self.inner().ref_count.fetch_sub(1, Ordering::Release) == 1 {
            std::sync::atomic::fence(Ordering::Acquire);
            unsafe { drop(Box::from_raw(self.ptr.as_ptr())); }
        }
    }
}
