use crate::sys::{self, emscripten_fetch_attr_t, emscripten_fetch_t};
use bitflags::bitflags;
use http::{Method, Response, StatusCode};
use std::{
    ffi::{c_char, c_void, CStr},
    fmt::Debug,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Deref,
    time::Duration,
};
use utils_atomics::flag::mpsc::{async_flag, AsyncFlag};

pub struct Builder<'a> {
    url: &'a CStr,
    attrs: emscripten_fetch_attr_t,
    headers: Vec<*const c_char>,
    _phtm: PhantomData<&'a mut &'a ()>,
}

impl<'a> Builder<'a> {
    pub fn new(method: Method, url: &'a CStr) -> Self {
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
            url,
            headers: Vec::new(),
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

    pub fn header(mut self, name: &'a CStr, value: &'a CStr) -> Self {
        self.headers.reserve(2);
        self.headers.push(name.as_ptr());
        self.headers.push(value.as_ptr());
        self
    }

    pub fn mime_type(mut self, mime: Option<&'a CStr>) -> Self {
        self.attrs.overriddenMimeType = match mime {
            Some(mime) => mime.as_ptr(),
            None => std::ptr::null(),
        };
        self
    }

    pub fn body(mut self, body: &'a [u8]) -> Self {
        self.attrs.requestData = body.as_ptr().cast();
        self.attrs.requestDataSize = body.len();
        self
    }

    pub async fn send(mut self) -> http::Result<Response<ResponseBody>> {
        unsafe extern "C" fn on_success(fetch: *mut emscripten_fetch_t) {
            let fetch = &mut *fetch;
            let handler = AsyncFlag::from_raw(fetch.userData.cast());
            handler.mark();
        }

        unsafe extern "C" fn on_error(fetch: *mut emscripten_fetch_t) {
            let fetch = &mut *fetch;
            let handler = AsyncFlag::from_raw(fetch.userData.cast());
            handler.mark();
        }

        let (send, recv) = async_flag();
        self.headers.push(std::ptr::null());

        self.attrs.attributes |= sys::EMSCRIPTEN_FETCH_LOAD_TO_MEMORY;
        self.attrs.requestHeaders = self.headers.as_ptr();
        self.attrs.userData = unsafe { send.into_raw() as *mut c_void };
        self.attrs.onsuccess = Some(on_success);
        self.attrs.onerror = Some(on_error);

        unsafe {
            let fetch = sys::emscripten_fetch(&mut self.attrs, self.url.as_ptr());
            assert!(!fetch.is_null());

            let fetch = &mut *fetch;
            let guard = CloseGuard(fetch);
            recv.await;

            let mut response = Response::builder().status(StatusCode::from_u16(fetch.status)?);

            // Read & unpack headers
            let mut header_len = sys::emscripten_fetch_get_response_headers_length(fetch) + 1;
            let mut headers = Vec::<u8>::with_capacity(header_len);
            header_len = sys::emscripten_fetch_get_response_headers(
                fetch,
                headers.as_mut_ptr().cast(),
                header_len,
            );
            headers.set_len(header_len);

            let mut remaining_header = headers.as_slice();
            while let Some(mut idx) = memchr::memchr(b'\n', remaining_header) {
                if remaining_header.get(idx + 1).is_some_and(|x| *x == b'\r') {
                    idx += 1;
                }

                let (header, rem) = remaining_header.split_at(idx);
                let header = &header[..header.len() - 1];

                remaining_header = &rem[1..];
                let delim = memchr::memchr(b':', header).unwrap_or(header.len());

                let (name, value) = header.split_at(delim);
                let value = trim_ascii_start(&value[1..]);

                response = response.header(name, value);
            }

            return response.body(ResponseBody { inner: guard });
        }
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

#[repr(transparent)]
pub struct ResponseBody {
    inner: CloseGuard,
}

impl Debug for ResponseBody {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.deref(), f)
    }
}

impl Deref for ResponseBody {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe {
            let fetch = &*self.inner.0;
            std::slice::from_raw_parts(
                fetch.data.cast(),
                usize::try_from(fetch.numBytes).unwrap_or(usize::MAX),
            )
        }
    }
}

#[repr(transparent)]
struct CloseGuard(*mut emscripten_fetch_t);

impl Drop for CloseGuard {
    #[inline]
    fn drop(&mut self) {
        unsafe { sys::emscripten_fetch_close(self.0) };
    }
}

const fn trim_ascii_start(mut bytes: &[u8]) -> &[u8] {
    // Note: A pattern matching based approach (instead of indexing) allows
    // making the function const.
    while let [first, rest @ ..] = bytes {
        if first.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }
    bytes
}
