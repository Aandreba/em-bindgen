use crate::sys::{self, emscripten_fetch_attr_t, emscripten_fetch_t};
use bitflags::bitflags;
use http::{Method, StatusCode};
use std::{
    ffi::{c_char, CStr},
    marker::PhantomData,
    mem::MaybeUninit,
    time::Duration,
};

pub struct Builder<'a> {
    attrs: emscripten_fetch_attr_t,
    _phtm: PhantomData<&'a mut &'a ()>,
}

impl<'a> Builder<'a> {
    pub fn new(method: Method) -> Self {
        let mut attrs = {
            let mut this = MaybeUninit::uninit();
            unsafe {
                sys::emscripten_fetch_attr_init(this.as_mut_ptr());
                this.assume_init()
            }
        };

        let method_name = method.as_str();
        attrs.requestMethod[..method_name.len()].copy_from_slice(unsafe {
            std::slice::from_raw_parts(method_name.as_ptr().cast::<c_char>(), method_name.len())
        });
        attrs.requestMethod[method_name.len()] = 0;

        return Self {
            attrs,
            _phtm: PhantomData,
        };
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.attrs.timeoutMSecs = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        self
    }

    pub fn attributes(mut self, attrs: FetchAttributes) -> Self {
        self.attrs.attributes = attrs.bits();
        self
    }

    pub fn mime_type(mut self, mime: Option<&'a CStr>) -> Self {
        self.attrs.overriddenMimeType = match mime {
            Some(mime) => mime.as_ptr(),
            None => std::ptr::null(),
        };
        self
    }

    pub fn send<H: FetchHandler>(mut self, url: &'a CStr, handler: H) {
        unsafe extern "C" fn on_success<H: FetchHandler>(fetch: *mut emscripten_fetch_t) {
            let _guard = CloseGuard(fetch);
            let (fetch, user_data) = Fetch::from_raw::<H>(fetch);
            let this = Box::from_raw(user_data);
            this.on_success(fetch)
        }

        unsafe extern "C" fn on_error<H: FetchHandler>(fetch: *mut emscripten_fetch_t) {
            let _guard = CloseGuard(fetch);
            let (fetch, user_data) = Fetch::from_raw::<H>(fetch);
            let this = Box::from_raw(user_data);
            this.on_error(fetch)
        }

        self.attrs.attributes |= sys::EMSCRIPTEN_FETCH_LOAD_TO_MEMORY;
        self.attrs.onsuccess = Some(on_success::<H>);
        self.attrs.onerror = Some(on_error::<H>);
        self.attrs.userData = Box::into_raw(Box::new(handler)).cast();

        unsafe {
            sys::emscripten_fetch(&mut self.attrs, url.as_ptr());
        }
    }

    pub fn send_streaming<H: FetchHandler>(mut self, url: &'a CStr, handler: H) {
        todo!()
    }
}

pub struct Fetch<'a> {
    pub id: u32,
    pub url: &'a CStr,
    pub data: Option<&'a [u8]>,
    pub data_offset: u64,
    pub total_bytes: u64,
    pub status: StatusCode,
}

impl<'a> Fetch<'a> {
    pub unsafe fn from_raw<T>(fetch: *mut emscripten_fetch_t) -> (Self, *mut T) {
        let fetch = &*fetch;
        return (
            Self {
                id: fetch.id,
                url: CStr::from_ptr(fetch.url),
                data: if fetch.data.is_null() {
                    None
                } else {
                    Some(std::slice::from_raw_parts(
                        fetch.data.cast(),
                        usize::try_from(fetch.numBytes).unwrap(),
                    ))
                },
                data_offset: fetch.dataOffset,
                total_bytes: fetch.totalBytes,
                status: StatusCode::from_u16(fetch.status).unwrap(),
            },
            fetch.userData.cast(),
        );
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(transparent)]
    pub struct FetchAttributes: u32 {
        const PERSIST_FILE = sys::EMSCRIPTEN_FETCH_PERSIST_FILE;
        // const SYNCHRONOUS = sys::EMSCRIPTEN_FETCH_SYNCHRONOUS;
        // const WAITABLE = sys::EMSCRIPTEN_FETCH_WAITABLE;
        // const APPEND = sys::EMSCRIPTEN_FETCH_APPEND;
        // const LOAD_TO_MEMORY = sys::EMSCRIPTEN_FETCH_LOAD_TO_MEMORY;
        // const NO_DOWNLOAD = sys::EMSCRIPTEN_FETCH_NO_DOWNLOAD;
        // const REPLACE = sys::EMSCRIPTEN_FETCH_REPLACE;
        // const STREAM_DATA = sys::EMSCRIPTEN_FETCH_STREAM_DATA;
    }
}

pub trait FetchHandler {
    fn on_success(self, fetch: Fetch);
    fn on_error(self, fetch: Fetch);
}

pub trait FetchProgressHandler: FetchHandler {
    fn on_progress<'a>(&'a mut self, fetch: &mut Fetch<'a>);
}

#[repr(transparent)]
struct CloseGuard(*mut emscripten_fetch_t);

impl Drop for CloseGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::emscripten_fetch_close(self.0) };
    }
}
