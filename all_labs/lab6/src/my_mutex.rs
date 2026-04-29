//! Власна реалізація Mutex через AtomicBool (spinlock).
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

/// Примітив взаємного виключення на основі spinlock.
pub struct MyMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for MyMutex<T> {}
unsafe impl<T: Send> Sync for MyMutex<T> {}

/// Охоронець — утримує блокування поки живий.
pub struct MyMutexGuard<'a, T> {
    mutex: &'a MyMutex<T>,
}

impl<T> MyMutex<T> {
    /// Створює новий `MyMutex` з переданим значенням.
    pub fn new(data: T) -> Self {
        Self { locked: AtomicBool::new(false), data: UnsafeCell::new(data) }
    }

    /// Захоплює блокування та повертає охоронець.
    pub fn lock(&self) -> MyMutexGuard<'_, T> {
        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
        MyMutexGuard { mutex: self }
    }
}

impl<T> Deref for MyMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.mutex.data.get() } }
}

impl<T> DerefMut for MyMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.mutex.data.get() } }
}

impl<T> Drop for MyMutexGuard<'_, T> {
    fn drop(&mut self) { self.mutex.locked.store(false, Ordering::Release); }
}
