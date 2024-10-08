use self::sys::fetch_header_t;
use crate::{
    fetch::sys::{fetch_attrs_t, fetch_status_t, GetResponseBytes, GetResponseChunks, SendRequest},
    future::block_on,
    value::JsValue,
};
use alloc::{alloc::Layout, borrow::Cow};
use core::{
    ffi::CStr, hint::unreachable_unchecked, marker::PhantomData, task::Poll, time::Duration,
};
use docfg::docfg;
use futures::{ready, Future, Stream, StreamExt};
use http::{Response, StatusCode};
use libc::c_void;
use libstd::io::{BufRead, Cursor, ErrorKind, Read};
use pin_project::pin_project;
use utils_atomics::channel::once::{async_channel, AsyncReceiver, AsyncSender};

pub async fn get(url: &CStr) -> Result<Response<ResponseBody>, RequestError> {
    Builder::new().send(Method::GET, url).await
}

pub struct Builder<'a> {
    timeout: Option<Duration>,
    headers: Vec<fetch_header_t>,
    body: Option<&'a [u8]>,
    _phtm: PhantomData<&'a [&'a CStr]>,
}

impl<'a> Builder<'a> {
    pub fn new() -> Self {
        return Self {
            timeout: None,
            headers: Vec::new(),
            body: None,
            _phtm: PhantomData,
        };
    }

    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn header(&mut self, key: &'a CStr, value: &'a CStr) -> &mut Self {
        self.headers.push(fetch_header_t {
            key: key.as_ptr(),
            value: value.as_ptr(),
        });
        self
    }

    pub fn body(&mut self, body: &'a [u8]) -> &mut Self {
        self.body = Some(body);
        self
    }

    pub async fn send(
        &self,
        method: Method,
        url: &CStr,
    ) -> Result<Response<ResponseBody>, RequestError> {
        unsafe extern "C" fn on_response(
            status: fetch_status_t,
            status_code: u16,
            headers: *const fetch_header_t,
            headers_len: usize,
            handle: *mut c_void,
            user_data: *mut c_void,
        ) {
            let send = Box::from_raw(
                user_data.cast::<AsyncSender<Result<Response<ResponseBody>, RequestError>>>(),
            );

            send.send(handle_response(
                status,
                status_code,
                std::slice::from_raw_parts(headers, headers_len),
                handle,
            ));
        }

        unsafe {
            let method = match method {
                Method::GET => c"GET",
                Method::HEAD => c"HEAD",
                Method::POST => c"POST",
                Method::PUT => c"PUT",
                Method::DELETE => c"DELETE",
                Method::CONNECT => c"CONNECT",
                Method::OPTIONS => c"OPTIONS",
                Method::TRACE => c"TRACE",
                Method::PATCH => c"PATCH",
            };

            let (body, body_len) = match self.body {
                Some(body) => (body.as_ptr(), body.len()),
                None => (std::ptr::null(), 0),
            };

            let attrs = fetch_attrs_t {
                timeout: u64::try_from(self.timeout.unwrap_or(Duration::ZERO).as_millis())
                    .unwrap_or(u64::MAX),
                headers: self.headers.as_ptr(),
                headers_len: self.headers.len(),
                body,
                body_len,
            };

            let (send, recv) = async_channel::<Result<Response<ResponseBody>, RequestError>>();
            SendRequest(
                method.as_ptr(),
                url.as_ptr(),
                attrs,
                Some(on_response),
                Box::into_raw(Box::new(send)).cast(),
            );

            return Ok(recv.await.transpose()?.unwrap());
        }
    }
}

#[derive(Debug)]
pub struct ResponseBody {
    inner: JsValue,
}

impl ResponseBody {
    pub async fn text(self) -> Result<String, RequestError> {
        self.bytes()
            .await
            .map(|b| match String::from_utf8_lossy(&b) {
                Cow::Borrowed(_) => unsafe { String::from_utf8_unchecked(b) },
                Cow::Owned(s) => s,
            })
    }

    pub async fn bytes(self) -> Result<Vec<u8>, RequestError> {
        unsafe extern "C" fn on_bytes_pre(len: usize, _: *mut c_void) -> *mut u8 {
            let Ok(layout) = Layout::array::<u8>(len) else {
                return std::ptr::null_mut();
            };
            return std::alloc::alloc(layout);
        }

        unsafe extern "C" fn on_bytes_post(
            status: fetch_status_t,
            ptr: *mut u8,
            len: usize,
            user_data: *mut c_void,
        ) {
            let send =
                Box::from_raw(user_data.cast::<AsyncSender<Result<Vec<u8>, RequestError>>>());

            send.send(match status {
                fetch_status_t::Sent => Ok(Vec::from_raw_parts(ptr, len, len)),
                fetch_status_t::TimedOut => Err(RequestError::TimedOut),
                fetch_status_t::Exception => Err(RequestError::Unexpected),
                fetch_status_t::Ended => unreachable_unchecked(),
            });
        }

        unsafe {
            let (send, recv) = async_channel::<Result<Vec<u8>, RequestError>>();
            GetResponseBytes(
                self.inner.as_handle().cast(),
                Some(on_bytes_pre),
                std::ptr::null_mut(),
                Some(on_bytes_post),
                Box::into_raw(Box::new(send)).cast(),
            );
            return recv.await.unwrap();
        }
    }

    pub fn chunks(self) -> ResponseChunks {
        unsafe extern "C" fn on_bytes_pre(len: usize, _: *mut c_void) -> *mut u8 {
            let Ok(layout) = Layout::array::<u8>(len) else {
                return std::ptr::null_mut();
            };
            return std::alloc::alloc(layout);
        }

        unsafe extern "C" fn on_bytes_post(
            status: fetch_status_t,
            ptr: *mut u8,
            len: usize,
            user_data: *mut c_void,
        ) {
            let user_data = user_data.cast::<AsyncSender<ResponseChunk>>();
            match status {
                fetch_status_t::Sent => {
                    let chunk = Vec::from_raw_parts(ptr, len, len);
                    let (new_send, new_recv) = async_channel();
                    std::mem::replace(&mut *user_data, new_send)
                        .send(ResponseChunk::Ok(chunk, new_recv));
                }
                fetch_status_t::TimedOut => {
                    let send = Box::from_raw(user_data);
                    send.send(ResponseChunk::Err(RequestError::TimedOut));
                }
                fetch_status_t::Exception => {
                    let send = Box::from_raw(user_data);
                    send.send(ResponseChunk::Err(RequestError::Unexpected));
                }
                fetch_status_t::Ended => drop(Box::from_raw(user_data)),
            }
        }
        unsafe {
            let (send, recv) = async_channel::<ResponseChunk>();
            GetResponseChunks(
                self.inner.as_handle().cast(),
                Some(on_bytes_pre),
                std::ptr::null_mut(),
                Some(on_bytes_post),
                Box::into_raw(Box::new(send)).cast(),
            );
            return ResponseChunks { recv: Some(recv) };
        }
    }

    #[inline]
    pub fn reader(self) -> ResponseReader {
        ResponseReader::new(self.chunks())
    }
}

#[pin_project]
pub struct ResponseChunks {
    #[pin]
    recv: Option<AsyncReceiver<ResponseChunk>>,
}

impl Stream for ResponseChunks {
    type Item = Result<Vec<u8>, RequestError>;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        let Some(recv) = this.recv.as_mut().as_pin_mut() else {
            return Poll::Ready(None);
        };

        match ready!(recv.poll(cx)) {
            Some(ResponseChunk::Ok(chunk, recv)) => {
                this.recv.set(Some(recv));
                return Poll::Ready(Some(Ok(chunk)));
            }
            Some(ResponseChunk::Err(e)) => {
                this.recv.set(None);
                return Poll::Ready(Some(Err(e)));
            }
            None => {
                this.recv.set(None);
                return Poll::Ready(None);
            }
        }
    }
}

pub struct ResponseReader {
    chunks: ResponseChunks,
    buffer: Option<Cursor<Vec<u8>>>,
}

impl ResponseReader {
    pub fn new(chunks: ResponseChunks) -> Self {
        Self {
            chunks,
            buffer: None,
        }
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, RequestError> {
        let mut len = 0;

        while len < buf.len() {
            // Read from the buffers
            if let Some(cursor) = self.buffer.as_mut() {
                len += cursor.read(&mut buf[len..])?;
                if cursor_is_empty(cursor) {
                    self.buffer = self.chunks.next().await.transpose()?.map(Cursor::new);
                } else {
                    break;
                }
            }
        }

        return Ok(len);
    }

    pub async fn fill_buf(&mut self) -> Result<&[u8], RequestError> {
        if !self.buffer.as_ref().is_some_and(|x| !cursor_is_empty(x)) {
            self.buffer = self.chunks.next().await.transpose()?.map(Cursor::new);
        }

        match self.buffer.as_mut() {
            Some(cursor) => cursor.fill_buf().map_err(RequestError::from),
            None => Ok(b""),
        }
    }

    #[inline]
    pub fn consume(&mut self, amt: usize) {
        if let Some(cursor) = self.buffer.as_mut() {
            cursor.consume(amt);
        }
    }
}

#[docfg(all(feature = "asyncify", feature = "proxying"))]
impl std::io::Read for ResponseReader {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> libstd::io::Result<usize> {
        block_on(self.read(buf)).map_err(std::io::Error::from)
    }
}

#[docfg(all(feature = "asyncify", feature = "proxying"))]
impl std::io::BufRead for ResponseReader {
    #[inline]
    fn fill_buf(&mut self) -> libstd::io::Result<&[u8]> {
        block_on(self.fill_buf()).map_err(std::io::Error::from)
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.consume(amt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Http(#[from] http::Error),
    #[error("The request timed out")]
    TimedOut,
    #[error("Unexpected error ocurred")]
    Unexpected,
}

impl From<RequestError> for std::io::Error {
    fn from(value: RequestError) -> Self {
        return match value {
            RequestError::Io(e) => e,
            RequestError::Http(e) => std::io::Error::new(ErrorKind::Other, e),
            RequestError::TimedOut => std::io::Error::from(ErrorKind::TimedOut),
            RequestError::Unexpected => std::io::Error::from(ErrorKind::Other),
        };
    }
}

enum ResponseChunk {
    Ok(Vec<u8>, AsyncReceiver<Self>),
    Err(RequestError),
}

unsafe fn handle_response(
    status: fetch_status_t,
    status_code: u16,
    headers: &[fetch_header_t],
    handle: *mut c_void,
) -> Result<Response<ResponseBody>, RequestError> {
    let response = match status {
        fetch_status_t::Sent => JsValue::take_ownership(handle.cast()),
        fetch_status_t::TimedOut => return Err(RequestError::TimedOut),
        fetch_status_t::Exception => return Err(RequestError::Unexpected),
        fetch_status_t::Ended => unreachable_unchecked(),
    };

    let mut builder = http::Response::builder()
        .status(StatusCode::from_u16(status_code).map_err(http::Error::from)?);

    for header in headers {
        let key = std::str::from_utf8_unchecked(CStr::from_ptr(header.key).to_bytes());
        let value = std::str::from_utf8_unchecked(CStr::from_ptr(header.value).to_bytes());
        builder = builder.header(key, value);
    }

    return builder
        .body(ResponseBody { inner: response })
        .map_err(RequestError::from);
}

#[inline]
fn cursor_is_empty(cursor: &Cursor<Vec<u8>>) -> bool {
    return cursor.position() >= cursor.get_ref().len() as u64;
}

pub mod sys {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]

    include!(concat!(env!("OUT_DIR"), "/fetch.rs"));
}
