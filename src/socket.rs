use crate::sys;
use std::ffi::c_void;
use std::os::fd::{BorrowedFd, RawFd};
use utils_atomics::AtomicCell;

static CURRENT_OPEN: AtomicCell<()> = 0;

#[doc(alias = "emscripten_set_socket_open_callback")]
pub fn set_socket_open_callback<F: Fn(BorrowedFd)>(mut f: F) {
    unsafe {
        sys::emscripten_set_socket_open_callback(closure.user_data(), Some(closure.fn_ptr()))
    };

    todo!()
}

unsafe extern "C" fn call_socket_callback<F: Fn() + Sync>(user_data: *mut c_void) {
    todo!()
}

unsafe extern "C" fn call_socket_error_callback() {
    todo!()
}
