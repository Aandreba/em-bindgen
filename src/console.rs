use crate::sys::*;
use core::ffi::CStr;
use log::Level;

#[inline]
pub fn console_log(s: &CStr) {
    unsafe { emscripten_console_log(s.as_ptr()) }
}

#[inline]
pub fn console_warn(s: &CStr) {
    unsafe { emscripten_console_warn(s.as_ptr()) }
}

#[inline]
pub fn console_error(s: &CStr) {
    unsafe { emscripten_console_error(s.as_ptr()) }
}
