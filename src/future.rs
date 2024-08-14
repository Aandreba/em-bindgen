#[cfg(feature = "proxying")]
use crate::proxying::Queue;
use crate::{
    set_timeout,
    sys::{self, em_promise_t, PthreadWrapper},
};
use docfg::docfg;
#[cfg(feature = "proxying")]
use futures::{
    executor::LocalPool,
    task::{LocalFutureObj, LocalSpawn, LocalSpawnExt},
};
use pin_project::pin_project;
use std::{
    any::Any,
    cell::{Cell, RefCell},
    ffi::c_void,
    future::Future,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::Deref,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Weak},
    task::{ready, Context, Poll, Waker},
    time::Duration,
};
use utils_atomics::{
    channel::once::{async_channel, AsyncReceiver},
    flag::mpsc::async_flag,
};

pub async fn sleep(dur: Duration) {
    let (send, recv) = async_flag();
    set_timeout(dur, move || send.mark());
    return recv.await;
}

#[docfg(all(feature = "asyncify", feature = "proxying"))]
pub fn block_on<Fut>(fut: Fut) -> Fut::Output
where
    Fut: Future,
{
    thread_local! {
        static CURRENT_EVENT: Cell<Option<Event<Inner>>> = Cell::new(None);
    }

    struct Wake {
        queue: Queue<'static>,
        thread: PthreadWrapper,
    }

    impl std::task::Wake for Wake {
        #[inline]
        fn wake(self: std::sync::Arc<Self>) {
            self.wake_by_ref()
        }

        fn wake_by_ref(self: &std::sync::Arc<Self>) {
            assert!(self.queue.proxy(&self.thread, move || {
                if let Some(current_event) = CURRENT_EVENT.take() {
                    let (new_event, promise) = event::<Inner>();
                    CURRENT_EVENT.set(Some(new_event));
                    current_event.fulfill(promise.into_raw());
                }
            }));
        }
    }

    let queue = Arc::new(Wake {
        queue: Queue::new(),
        thread: PthreadWrapper::current(),
    });

    let waker = Waker::from(queue.clone());
    let mut cx = Context::from_waker(&waker);

    let (event, mut promise) = event::<Inner>();
    CURRENT_EVENT.set(Some(event));

    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(val) => return val,
            std::task::Poll::Pending => unsafe { promise = Promise::from_raw(promise.block_on()) },
        }
    }
}

#[docfg(feature = "proxying")]
pub fn spawn_local<Fut>(fut: Fut) -> JoinHandle<Fut::Output>
where
    Fut: 'static + Future,
    Fut::Output: 'static,
{
    #[pin_project]
    struct Task<Fut> {
        #[pin]
        fut: Fut,
        queue: Weak<Queue<'static>>,
        _phtm: PhantomData<*mut ()>,
    }

    impl<Fut: Future<Output = ()>> Future for Task<Fut> {
        type Output = ();

        fn poll(
            self: std::pin::Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            let this = self.project();

            struct Wake {
                parent: Waker,
                queue: Weak<Queue<'static>>,
                thread: PthreadWrapper,
            }

            impl std::task::Wake for Wake {
                #[inline]
                fn wake(self: std::sync::Arc<Self>) {
                    self.wake_by_ref()
                }

                fn wake_by_ref(self: &std::sync::Arc<Self>) {
                    self.parent.wake_by_ref();
                    if let Some(queue) = self.queue.upgrade() {
                        queue.proxy(&self.thread, move || LOCAL_RUNTIME.with(|rt| rt.enqueue()));
                    }
                }
            }

            let waker = Waker::from(Arc::new(Wake {
                parent: cx.waker().clone(),
                queue: this.queue.clone(),
                thread: PthreadWrapper::current(),
            }));
            let mut cx = Context::from_waker(&waker);
            return this.fut.poll(&mut cx);
        }
    }

    struct Pool {
        pool: RefCell<LocalPool>,
        backlog: RefCell<Vec<LocalFutureObj<'static, ()>>>,
    }

    impl Pool {
        fn push(&self, queue: &Arc<Queue<'static>>, fut: impl 'static + Future<Output = ()>) {
            let fut = Task {
                fut,
                queue: Arc::downgrade(queue),
                _phtm: PhantomData,
            };

            if let Some(pool) = self.pool.try_borrow_mut().ok() {
                pool.spawner()
                    .spawn_local(fut)
                    .expect("Error spawning local task");
            } else {
                self.backlog
                    .borrow_mut()
                    .push(LocalFutureObj::new(Box::new(fut)));
            }
        }
    }

    struct Runtime {
        queue: Arc<Queue<'static>>,
        tasks: Pool,
        is_running: Cell<bool>,
    }

    impl Runtime {
        pub fn new() -> Self {
            return Runtime {
                queue: Arc::new(Queue::new()),
                tasks: Pool {
                    pool: RefCell::new(LocalPool::new()),
                    backlog: RefCell::new(Vec::new()),
                },
                is_running: Cell::new(false),
            };
        }

        pub fn spawn<Fut: 'static + Future>(&self, fut: Fut) -> JoinHandle<Fut::Output> {
            use futures::FutureExt;
            use std::panic::AssertUnwindSafe;

            let (send, recv) = async_channel();
            self.tasks.push(&self.queue, async move {
                send.send(AssertUnwindSafe(fut).catch_unwind().await)
            });
            self.enqueue();
            return JoinHandle { recv };
        }

        fn enqueue(&self) {
            if !self.is_running.replace(true) {
                assert!(self
                    .queue
                    .proxy_local(|| LOCAL_RUNTIME.with(|rt| rt.poll())));
            }
        }

        fn poll(&self) {
            let mut pool = RefCell::borrow_mut(&self.tasks.pool);
            pool.run_until_stalled();
            self.is_running.set(false);

            let spawner = pool.spawner();
            for future in self.tasks.backlog.borrow_mut().drain(..) {
                spawner
                    .spawn_local_obj(future)
                    .expect("Error spawning local task");
            }
        }
    }

    thread_local! {
        static LOCAL_RUNTIME: Runtime = Runtime::new();
    }

    return LOCAL_RUNTIME.with(|rt| rt.spawn(fut));
}

pub struct JoinHandle<T> {
    pub(crate) recv: AsyncReceiver<Result<T, Box<dyn Any + Send + 'static>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum JoinError {
    #[error("The task was aborted")]
    Aborted,
    #[error("The task panicked")]
    Panic(Box<dyn Any + Send + 'static>),
}

impl<T> JoinHandle<T> {
    pub fn abort(self) {
        todo!()
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        return Poll::Ready(match ready!(Pin::new(&mut self.recv).poll(cx)) {
            Some(Ok(val)) => Ok(val),
            Some(Err(e)) => Err(JoinError::Panic(e)),
            None => Err(JoinError::Aborted),
        });
    }
}

pub fn event<T>() -> (Event<T>, Promise<T>) {
    let raw = Rc::new(RawPromise::new());

    return (
        Event {
            raw: raw.clone(),
            _phtm: PhantomData,
        },
        Promise {
            raw: ManuallyDrop::new(Inner::Shared(raw)),
            _phtm: PhantomData,
        },
    );
}

pub struct Event<T> {
    raw: Rc<RawPromise>,
    _phtm: PhantomData<T>,
}

impl<T> Event<T> {
    #[inline]
    pub fn fulfill(self, val: T) {
        unsafe { self.fulfill_ref(val) };
    }

    #[inline]
    pub unsafe fn fulfill_ref(&self, val: T) {
        unsafe { self.raw.fulfill(val) };
    }
}

pub struct Promise<T> {
    raw: ManuallyDrop<Inner>,
    _phtm: PhantomData<T>,
}

impl<T> Promise<T> {
    pub fn into_raw(self) -> Inner {
        let mut this = ManuallyDrop::new(self);
        return unsafe { ManuallyDrop::take(&mut this.raw) };
    }

    pub unsafe fn from_raw(raw: Inner) -> Self {
        return Self {
            raw: ManuallyDrop::new(raw),
            _phtm: PhantomData,
        };
    }

    #[docfg(feature = "asyncify")]
    #[inline]
    pub fn block_on(self) -> T {
        return *self.block_on_boxed();
    }

    #[docfg(feature = "asyncify")]
    pub fn block_on_boxed(self) -> Box<T> {
        let mut this = ManuallyDrop::new(self);
        let result = this.raw.block_on();
        unsafe { ManuallyDrop::drop(&mut this.raw) };
        debug_assert_eq!(result.result, sys::em_promise_result_t::EM_PROMISE_FULFILL);
        return unsafe { Box::from_raw(result.value.cast::<T>()) };
    }
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        let _ = self.raw.then(move |data| unsafe {
            drop(Box::from_raw(data.cast::<T>()));
            return sys::em_settled_result_t {
                result: sys::em_promise_result_t::EM_PROMISE_FULFILL,
                value: std::ptr::null_mut(),
            };
        });

        unsafe {
            ManuallyDrop::drop(&mut self.raw);
        }
    }
}

enum Inner {
    Shared(Rc<RawPromise>),
    // Owned(RawPromise),
}

impl Deref for Inner {
    type Target = RawPromise;

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Inner::Shared(raw) => raw,
            // Inner::Owned(raw) => raw,
        }
    }
}

#[repr(transparent)]
struct RawPromise {
    inner: em_promise_t,
}

impl RawPromise {
    fn new() -> Self {
        return Self {
            inner: unsafe { sys::emscripten_promise_create() },
        };
    }

    fn then<F>(&self, f: F) -> Self
    where
        F: 'static + FnOnce(*mut c_void) -> sys::em_settled_result_t,
    {
        unsafe extern "C" fn then<F: FnOnce(*mut c_void) -> sys::em_settled_result_t>(
            result: *mut *mut c_void,
            data: *mut c_void,
            value: *mut c_void,
        ) -> sys::em_promise_result_t {
            let f = Box::from_raw(data.cast::<F>());
            let settled_result = f(value);

            *result = settled_result.value;
            return settled_result.result;
        }

        let data = Box::new(f);
        return Self {
            inner: unsafe {
                sys::emscripten_promise_then(
                    self.inner,
                    Some(then::<F>),
                    None,
                    Box::into_raw(data).cast(),
                )
            },
        };
    }

    #[inline]
    unsafe fn fulfill<T>(&self, val: T) {
        self.fulfill_boxed(Box::new(val))
    }

    unsafe fn fulfill_boxed<T>(&self, val: Box<T>) {
        unsafe {
            sys::emscripten_promise_resolve(
                self.inner,
                sys::em_promise_result_t::EM_PROMISE_FULFILL,
                Box::into_raw(val).cast(),
            )
        }
    }

    #[cfg(feature = "asyncify")]
    fn block_on(&self) -> sys::em_settled_result_t {
        unsafe { sys::emscripten_promise_await(self.inner) }
    }
}

impl Drop for RawPromise {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::emscripten_promise_destroy(self.inner) }
    }
}
