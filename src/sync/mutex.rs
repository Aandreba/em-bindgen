use crate::future::block_on;
use alloc::collections::VecDeque;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};
use utils_atomics::flag::mpsc::{async_flag, AsyncFlag};

/// A Mutex implementation designed to block the thread as little as possible, instead yielding back to the JavaScript runtime whenever possible.
pub struct Mutex<T: ?Sized> {
    raw: RawMutex,
    inner: UnsafeCell<T>,
}

impl<T: ?Sized> Mutex<T> {
    pub const fn new(val: T) -> Self
    where
        T: Sized,
    {
        Self {
            raw: RawMutex::new(),
            inner: UnsafeCell::new(val),
        }
    }

    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        self.raw.try_lock().then(|| MutexGuard { parent: self })
    }

    #[inline]
    pub fn lock(&self) -> MutexGuard<T> {
        self.raw.lock();
        return MutexGuard { parent: self };
    }
}

pub struct MutexGuard<'a, T: ?Sized> {
    parent: &'a Mutex<T>,
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.parent.inner.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.parent.inner.get() }
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe { self.parent.raw.unlock() }
    }
}

// these are the only places where `T: Send` matters; all other
// functionality works fine on a single thread.
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

pub(super) struct RawMutex {
    locked: AtomicBool,
    waiters: Waiters,
}

impl RawMutex {
    pub const fn new() -> Self {
        return Self {
            locked: AtomicBool::new(false),
            waiters: Waiters::new(),
        };
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        self.locked
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    pub fn lock(&self) {
        while !self.try_lock() {
            let (send, recv) = async_flag();
            self.waiters.push(send);
            block_on(recv);
        }
    }

    pub unsafe fn unlock(&self) {
        self.locked.store(false, Ordering::AcqRel);
        if let Some(waiter) = self.waiters.pop() {
            waiter.mark();
        }
    }
}

struct Waiters {
    busy: AtomicBool,
    waiters: UnsafeCell<VecDeque<AsyncFlag>>,
}

impl Waiters {
    const fn new() -> Self {
        Self {
            busy: AtomicBool::new(false),
            waiters: UnsafeCell::new(VecDeque::new()),
        }
    }

    #[inline]
    fn push(&self, evt: AsyncFlag) {
        self.wait();
        unsafe { &mut *self.waiters.get() }.push_back(evt);
        self.busy.store(false, Ordering::Release);
    }

    #[inline]
    fn pop(&self) -> Option<AsyncFlag> {
        self.wait();
        let res = unsafe { &mut *self.waiters.get() }.pop_front();
        self.busy.store(false, Ordering::Release);
        return res;
    }

    #[inline]
    fn wait(&self) {
        while self
            .busy
            .compare_exchange_weak(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
    }
}

unsafe impl Send for Waiters {}
unsafe impl Sync for Waiters {}
