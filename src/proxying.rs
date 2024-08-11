use crate::sys;
use std::{
    ffi::c_void, marker::PhantomData, mem::transmute, os::unix::thread::JoinHandleExt, sync::Arc,
};
use utils_atomics::TakeCell;

pub struct Queue<'a> {
    inner: *mut sys::em_proxying_queue,
    _phtm: PhantomData<&'a mut &'a ()>,
}

impl<'a> Queue<'a> {
    pub fn new() -> Self {
        return Self {
            inner: unsafe { sys::em_proxying_queue_create() },
            _phtm: PhantomData,
        };
    }

    pub fn proxy<F>(&self, target_thread: &impl JoinHandleExt, f: F) -> bool
    where
        F: 'a + FnOnce() + Send,
    {
        unsafe extern "C" fn proxy<F: FnOnce()>(arg: *mut c_void) {
            Box::from_raw(arg.cast::<F>())();
        }

        let arg = Box::into_raw(Box::new(f));
        unsafe {
            if sys::emscripten_proxy_async(
                self.inner,
                transmute::<std::os::unix::thread::RawPthread, sys::pthread_t>(
                    target_thread.as_pthread_t(),
                ),
                Some(proxy::<F>),
                arg.cast(),
            ) == 0
            {
                drop(Box::from_raw(arg));
                return false;
            }
            return true;
        }
    }

    pub fn proxy_local<F>(&self, f: F) -> bool
    where
        F: 'a + FnOnce(),
    {
        unsafe extern "C" fn proxy<F: FnOnce()>(arg: *mut c_void) {
            Box::from_raw(arg.cast::<F>())();
        }

        let arg = Box::into_raw(Box::new(f));
        unsafe {
            if sys::emscripten_proxy_async(
                self.inner,
                libc::pthread_self(),
                Some(proxy::<F>),
                arg.cast(),
            ) == 0
            {
                drop(Box::from_raw(arg));
                return false;
            }
            return true;
        }
    }

    pub fn proxy_blocking<F, T>(&self, target_thread: &impl JoinHandleExt, f: F) -> Option<T>
    where
        F: FnOnce() -> T + Send,
        T: Send,
    {
        struct Proxy<F, T> {
            result: Option<T>,
            f: Option<F>,
        }

        unsafe extern "C" fn proxy<F: FnOnce() -> T, T>(arg: *mut c_void) {
            let proxy = &mut *arg.cast::<Proxy<F, T>>();
            proxy.result = Some((proxy.f.take().unwrap_unchecked())());
        }

        let target_thread = target_thread.as_pthread_t();
        if target_thread == unsafe { libc::pthread_self() } {
            return Some(f());
        }

        let arg = Box::into_raw(Box::new(Proxy {
            result: None,
            f: Some(f),
        }));

        unsafe {
            let result = sys::emscripten_proxy_sync(
                self.inner,
                target_thread,
                Some(proxy::<F, T>),
                arg.cast(),
            ) == 0;

            let arg = Box::from_raw(arg);
            if result {
                return arg.result;
            } else {
                return None;
            }
        }
    }

    pub fn proxy_blocking_with_ctx<F>(&self, target_thread: &impl JoinHandleExt, f: F) -> bool
    where
        F: FnOnce(Context) + Send,
    {
        unsafe extern "C" fn proxy<F: FnOnce(Context)>(
            ctx: *mut sys::em_proxying_ctx,
            arg: *mut c_void,
        ) {
            Box::from_raw(arg.cast::<F>())(Context {
                inner: ctx,
                _phtm: PhantomData,
            });
        }

        let arg = Box::into_raw(Box::new(f));
        unsafe {
            return sys::emscripten_proxy_sync_with_ctx(
                self.inner,
                transmute::<std::os::unix::thread::RawPthread, sys::pthread_t>(
                    target_thread.as_pthread_t(),
                ),
                Some(proxy::<F>),
                arg.cast(),
            ) != 0;
        }
    }

    pub fn proxy_callback<C>(&self, target_thread: &impl JoinHandleExt, cb: C) -> bool
    where
        C: IntoCallback<Callback: 'a>,
        <C::Callback as Callback>::Receiver: 'a,
    {
        struct Inner<C: Callback> {
            cb: Option<C>,
            recv: Option<C::Receiver>,
        }

        unsafe extern "C" fn proxy<C: Callback>(arg: *mut c_void) {
            let this = &mut *arg.cast::<Inner<C>>();
            this.cb.take().unwrap_unchecked().call();
        }

        unsafe extern "C" fn callback<C: Callback>(arg: *mut c_void) {
            let mut this = Box::from_raw(arg.cast::<Inner<C>>());
            C::callback(this.recv.take().unwrap_unchecked());
        }

        unsafe extern "C" fn cancel<C: Callback>(arg: *mut c_void) {
            let mut this = Box::from_raw(arg.cast::<Inner<C>>());
            C::cancel(this.recv.take().unwrap_unchecked());
        }

        let (cb, recv) = cb.into_callback();
        let arg = Box::into_raw(Box::new(Inner {
            cb: Some(cb),
            recv: Some(recv),
        }));

        unsafe {
            if sys::emscripten_proxy_callback(
                self.inner,
                transmute::<std::os::unix::thread::RawPthread, sys::pthread_t>(
                    target_thread.as_pthread_t(),
                ),
                Some(proxy::<C::Callback>),
                Some(callback::<C::Callback>),
                Some(cancel::<C::Callback>),
                arg.cast(),
            ) == 0
            {
                drop(Box::from_raw(arg));
                return false;
            }
            return true;
        }
    }

    pub fn proxy_callback_with_ctx<C>(&self, target_thread: &impl JoinHandleExt, cb: C) -> bool
    where
        C: IntoCallbackWithCtx<Callback: 'a>,
        <C::Callback as CallbackWithCtx>::Receiver: 'a,
    {
        struct Inner<C: CallbackWithCtx> {
            cb: TakeCell<C>,
            recv: TakeCell<C::Receiver>,
        }

        impl<C: CallbackWithCtx> Inner<C> {
            fn new(cb: C, recv: C::Receiver) -> Self {
                return Self {
                    cb: TakeCell::new(cb),
                    recv: TakeCell::new(recv),
                };
            }
        }

        unsafe extern "C" fn proxy<C: CallbackWithCtx>(
            ctx: *mut sys::em_proxying_ctx,
            arg: *mut c_void,
        ) {
            let this = Arc::from_raw(arg as *const Inner<C>);
            this.cb.try_take().unwrap_unchecked().call(Context {
                inner: ctx,
                _phtm: PhantomData,
            });
        }

        unsafe extern "C" fn callback<C: CallbackWithCtx>(arg: *mut c_void) {
            let this = Arc::from_raw(arg as *const Inner<C>);
            C::callback(this.recv.try_take().unwrap_unchecked());
        }

        unsafe extern "C" fn cancel<C: CallbackWithCtx>(arg: *mut c_void) {
            let this = Arc::from_raw(arg as *const Inner<C>);
            C::cancel(this.recv.try_take().unwrap_unchecked());
        }

        let (cb, recv) = cb.into_callback_with_ctx();
        let arg = Arc::into_raw(Arc::new(Inner::new(cb, recv)));

        unsafe {
            Arc::increment_strong_count(arg);
            if sys::emscripten_proxy_callback_with_ctx(
                self.inner,
                transmute::<std::os::unix::thread::RawPthread, sys::pthread_t>(
                    target_thread.as_pthread_t(),
                ),
                Some(proxy::<C::Callback>),
                Some(callback::<C::Callback>),
                Some(cancel::<C::Callback>),
                arg as *mut c_void,
            ) == 0
            {
                Arc::decrement_strong_count(arg);
                Arc::decrement_strong_count(arg);
                return false;
            }
            return true;
        }
    }

    /// Execute all the tasks enqueued for the current thread on the given queue. New tasks that are enqueued concurrently with this execution will be executed as well. This function returns once it observes an empty queue.
    pub fn execute(&self) {
        unsafe { sys::emscripten_proxy_execute_queue(self.inner) }
    }
}

impl Drop for Queue<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::em_proxying_queue_destroy(self.inner) }
    }
}

unsafe impl Sync for Queue<'_> {}
unsafe impl Send for Queue<'_> {}

pub trait IntoCallback {
    type Callback: Callback;

    fn into_callback(self) -> (Self::Callback, <Self::Callback as Callback>::Receiver);
}

pub trait Callback: Send {
    type Receiver;

    fn call(self);
    fn callback(this: Self::Receiver);
    fn cancel(this: Self::Receiver);
}

pub trait IntoCallbackWithCtx {
    type Callback: CallbackWithCtx;

    fn into_callback_with_ctx(
        self,
    ) -> (
        Self::Callback,
        <Self::Callback as CallbackWithCtx>::Receiver,
    );
}

pub trait CallbackWithCtx: Send {
    type Receiver;

    fn call(self, ctx: Context);
    fn callback(this: Self::Receiver);
    fn cancel(this: Self::Receiver);
}

impl<C: Callback> IntoCallback for (C, C::Receiver) {
    type Callback = C;

    #[inline(always)]
    fn into_callback(self) -> (Self::Callback, <Self::Callback as Callback>::Receiver) {
        self
    }
}

impl<C: CallbackWithCtx> IntoCallbackWithCtx for (C, C::Receiver) {
    type Callback = C;

    #[inline(always)]
    fn into_callback_with_ctx(
        self,
    ) -> (
        Self::Callback,
        <Self::Callback as CallbackWithCtx>::Receiver,
    ) {
        self
    }
}

pub struct Context<'a> {
    inner: *mut sys::em_proxying_ctx,
    _phtm: PhantomData<&'a ()>,
}

impl<'a> Context<'a> {
    #[inline]
    pub fn finish(self) {}
}

impl Drop for Context<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::emscripten_proxy_finish(self.inner) }
    }
}
