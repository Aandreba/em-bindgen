#![cfg_attr(docsrs, feature(doc_cfg))]

use docfg::docfg;
use semver::Version;
use std::{
    ffi::{c_int, c_long, c_void, CStr},
    num::NonZeroU32,
    time::Duration,
};

#[doc(inline)]
pub use sys::emscripten_cancel_main_loop as cancel_main_loop;
#[doc(inline)]
pub use sys::emscripten_get_device_pixel_ratio as get_device_pixel_ratio;
#[doc(inline)]
pub use sys::emscripten_get_now as get_now;
#[doc(inline)]
pub use sys::emscripten_hide_mouse as hide_mouse;
#[doc(inline)]
pub use sys::emscripten_random as random;

#[cfg(feature = "fetch")]
#[cfg_attr(docsrs, doc(cfg(feature = "fetch")))]
pub mod fetch;
pub mod future;
#[cfg(feature = "proxying")]
#[cfg_attr(docsrs, doc(cfg(feature = "proxying")))]
pub mod proxying;

pub const EMSCRIPTEN_VERSION: Version = Version::new(
    sys::__EMSCRIPTEN_major__ as u64,
    sys::__EMSCRIPTEN_minor__ as u64,
    sys::__EMSCRIPTEN_tiny__ as u64,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timing {
    /// Specifies the time to wait between subsequent ticks to the main loop and updates occur independent of the vsync rate of the display (vsync off). This method uses the JavaScript [`setTimeout`](https://developer.mozilla.org/en-US/docs/Web/API/setTimeout) function to drive the animation.
    SetTimeout(Duration),
    /// Updates are performed using the setImmediate function, or if not available, emulated via postMessage. See [`setImmediate`](https://developer.mozilla.org/en-US/docs/Web/API/Window/setImmediate) for more information. Note that this mode is **strongly not recommended** to be used when deploying Emscripten output to the web, since it depends on an unstable web extension that is in draft status, browsers other than IE do not currently support it, and its implementation has been considered controversial in review.
    #[deprecated(
        note = "setImmediate is deprecated (or was never implemented) on all major browsers/environments"
    )]
    SetImmediate,
    /// Updates are performed using the [`requestAnimationFrame`](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame) function (with vsync enabled), and this value is interpreted as a "swap interval" rate for the main loop. The value of 1 specifies the runtime that it should render at every vsync (typically 60fps), whereas the value 2 means that the main loop callback should be called only every second vsync (30fps). As a general formula, the value n means that the main loop is updated at every n’th vsync, or at a rate of 60/n for 60Hz displays, and 120/n for 120Hz displays.
    Raf(NonZeroU32),
}

impl From<Duration> for Timing {
    #[inline]
    fn from(value: Duration) -> Self {
        Self::SetTimeout(value)
    }
}

/// See [Emscripten documentation](https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_async_call)
#[doc(alias = "emscripten_async_call")]
pub fn set_timeout<F: 'static + FnOnce()>(dur: Duration, f: F) {
    unsafe extern "C" fn timeout<F: FnOnce()>(arg: *mut c_void) {
        Box::from_raw(arg.cast::<F>())()
    }

    let millis = c_int::try_from(dur.as_millis()).unwrap_or(c_int::MAX);
    let arg = Box::into_raw(Box::new(f));
    unsafe { sys::emscripten_async_call(Some(timeout::<F>), arg.cast(), millis) }
}

/// See [Emscripten documentation](https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_set_main_loop)
#[doc(alias = "emscripten_set_main_loop")]
pub fn set_main_loop<F: FnMut()>(mut f: F, timing: Option<Timing>, simulate_infinite_loop: bool) {
    unsafe extern "C" fn main_loop<F: FnMut()>(arg: *mut c_void) {
        (&mut *arg.cast::<F>())()
    }

    #[inline(always)]
    fn main_loop_of<F: FnMut()>(_: &F) -> unsafe extern "C" fn(*mut c_void) {
        return main_loop::<F>;
    }

    unsafe {
        if let Some(timing) = timing {
            let mut first_call = true;
            let mut f = move || {
                if std::mem::take(&mut first_call) {
                    set_main_loop_timing(timing);
                }
                f();
            };

            sys::emscripten_set_main_loop_arg(
                Some(main_loop_of(&f)),
                std::ptr::addr_of_mut!(f).cast(),
                0,
                simulate_infinite_loop as c_int,
            );
        } else {
            sys::emscripten_set_main_loop_arg(
                Some(main_loop::<F>),
                std::ptr::addr_of_mut!(f).cast(),
                0,
                simulate_infinite_loop as c_int,
            );
        }
    }
}

/// See [Emscripten documentation](https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_set_main_loop_timing)
#[doc(alias = "emscripten_set_main_loop_timing")]
pub fn set_main_loop_timing(timing: Timing) {
    let (mode, value) = match timing {
        Timing::SetTimeout(dur) => {
            let millis = c_int::try_from(dur.as_millis()).unwrap_or(c_int::MAX);
            (sys::EM_TIMING_SETTIMEOUT as c_int, millis)
        }
        #[allow(deprecated)]
        Timing::SetImmediate => (sys::EM_TIMING_SETIMMEDIATE as c_int, 0),
        Timing::Raf(val) => (
            sys::EM_TIMING_RAF as c_int,
            c_int::try_from(val.get()).unwrap_or(c_int::MAX),
        ),
    };

    unsafe {
        sys::emscripten_set_main_loop_timing(mode, value);
    }
}

/// See [Emscripten documentation](https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_sleep)
#[docfg(feature = "asyncify")]
#[doc(alias = "emscripten_sleep")]
pub fn sleep(dur: std::time::Duration) {
    use std::ffi::c_uint;

    const LIMIT: u128 = c_uint::MAX as u128;

    let millis = dur.as_millis();
    let div = millis / LIMIT;
    let rem = millis % LIMIT;

    unsafe {
        sys::emscripten_sleep(rem as c_uint);
        for _ in 0..div {
            sys::emscripten_sleep(c_uint::MAX);
        }
    }
}

/// See [Emscripten documentation](https://emscripten.org/docs/api_reference/emscripten.h.html#c.emscripten_get_compiler_setting)
#[doc(alias = "emscripten_get_compiler_setting")]
#[inline]
pub fn get_compiler_setting(name: &CStr) -> c_long {
    return unsafe { sys::emscripten_get_compiler_setting(name.as_ptr()) };
}

pub mod sys {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]

    use std::os::unix::thread::JoinHandleExt;
    pub use std::os::unix::thread::RawPthread as pthread_t;

    include!(concat!(env!("OUT_DIR"), "/emscripten.rs"));

    #[derive(Clone, Copy)]
    #[repr(transparent)]
    pub(crate) struct PthreadWrapper(pub pthread_t);

    impl PthreadWrapper {
        pub fn current() -> Self {
            return Self(unsafe { libc::pthread_self() });
        }
    }

    impl JoinHandleExt for PthreadWrapper {
        #[inline]
        fn as_pthread_t(&self) -> std::os::unix::thread::RawPthread {
            self.into_pthread_t()
        }

        #[inline]
        fn into_pthread_t(self) -> std::os::unix::thread::RawPthread {
            self.0
        }
    }

    unsafe impl Send for PthreadWrapper {}
    unsafe impl Sync for PthreadWrapper {}
}

// pub mod glue {
//     #![allow(non_upper_case_globals)]
//     #![allow(non_camel_case_types)]
//     #![allow(non_snake_case)]
//
//     include!(concat!(env!("OUT_DIR"), "/glue.rs"));
// }
