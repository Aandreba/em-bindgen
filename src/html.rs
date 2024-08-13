use crate::sys::{self, *};
use core::ffi::{c_int, CStr};
use libc::{c_char, c_void};

const EMSCRIPTEN_EVENT_TARGET_DOCUMENT: *const c_char = 1usize as *const c_char;
const EMSCRIPTEN_EVENT_TARGET_WINDOW: *const c_char = 2usize as *const c_char;
const EMSCRIPTEN_EVENT_TARGET_SCREEN: *const c_char = 3usize as *const c_char;

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_get_element_css_size)
#[doc(alias = "emscripten_get_element_css_size")]
#[inline]
pub fn get_element_css_size(target: &CStr) -> [f64; 2] {
    let mut size = [0.; 2];
    unsafe { sys::emscripten_get_element_css_size(target.as_ptr(), &mut size[0], &mut size[1]) };
    return size;
}

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_set_canvas_size)
#[doc(alias = "emscripten_set_canvas_size")]
#[inline]
pub fn set_canvas_size(width: c_int, height: c_int) {
    unsafe { sys::emscripten_set_canvas_size(width, height) };
}

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_get_canvas_element_size)
#[doc(alias = "emscripten_get_canvas_element_size")]
#[inline]
pub fn get_canvas_element_size(target: &CStr) -> [c_int; 2] {
    let mut size = [0; 2];
    unsafe { sys::emscripten_get_canvas_element_size(target.as_ptr(), &mut size[0], &mut size[1]) };
    return size;
}

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_set_canvas_element_size)
#[doc(alias = "emscripten_set_canvas_element_size")]
#[inline]
pub fn set_canvas_element_size(target: &CStr, width: c_int, height: c_int) {
    unsafe { sys::emscripten_set_canvas_element_size(target.as_ptr(), width, height) };
}

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_set_fullscreenchange_callback)
#[doc(alias = "emscripten_set_fullscreenchange_callback")]
pub fn set_fullscreenchange_callback<'a, F>(
    target: impl Into<Target<'a>>,
    use_capture: bool,
    f: F,
) -> Result<(), HtmlError>
where
    F: 'static + FnMut(FullscreenChangeEvent),
{
    unsafe extern "C" fn fullscreenchange<F: 'static + FnMut(FullscreenChangeEvent)>(
        _: c_int,
        event: *const EmscriptenFullscreenChangeEvent,
        user_data: *mut c_void,
    ) -> c_int {
        let event = &*event;
        (&mut *user_data.cast::<F>())(FullscreenChangeEvent {
            is_fullscreen: event.isFullscreen != EM_FALSE as c_int,
            fullscreen_enabled: event.fullscreenEnabled != EM_FALSE as c_int,
            node_name: CStr::from_ptr(event.nodeName.as_ptr()),
            id: CStr::from_ptr(event.id.as_ptr()),
            element_width: event.elementWidth,
            element_height: event.elementHeight,
            screen_width: event.screenWidth,
            screen_height: event.screenHeight,
        });
        return EM_TRUE as c_int;
    }

    let f = Box::into_raw(Box::new(f));
    if let Err(e) = tri(unsafe {
        sys::emscripten_set_fullscreenchange_callback_on_thread(
            target.into().get(),
            f.cast(),
            use_capture as c_int,
            Some(fullscreenchange::<F>),
            libc::pthread_self(),
        )
    }) {
        drop(unsafe { Box::from_raw(f) });
        return Err(e);
    }
    return Ok(());
}

/// See [Emscripten docs](https://emscripten.org/docs/api_reference/html5.h.html#c.emscripten_set_resize_callback)
#[doc(alias = "emscripten_set_resize_callback")]
pub fn set_resize_callback<'a, F>(
    target: impl Into<Target<'a>>,
    use_capture: bool,
    f: F,
) -> Result<(), HtmlError>
where
    F: 'static + FnMut(UiEvent),
{
    unsafe extern "C" fn fullscreenchange<F: 'static + FnMut(UiEvent)>(
        _: c_int,
        event: *const EmscriptenUiEvent,
        user_data: *mut c_void,
    ) -> c_int {
        let event = &*event;
        (&mut *user_data.cast::<F>())(UiEvent {
            detail: event.detail,
            document_body_client_width: event.documentBodyClientWidth,
            document_body_client_height: event.documentBodyClientHeight,
            window_inner_width: event.windowInnerWidth,
            window_inner_height: event.windowInnerHeight,
            window_outer_width: event.windowOuterWidth,
            window_outer_height: event.windowOuterHeight,
            scroll_top: event.scrollTop,
            scroll_left: event.scrollLeft,
        });
        return EM_TRUE as c_int;
    }

    let f = Box::into_raw(Box::new(f));
    if let Err(e) = tri(unsafe {
        sys::emscripten_set_resize_callback_on_thread(
            target.into().get(),
            f.cast(),
            use_capture as c_int,
            Some(fullscreenchange::<F>),
            libc::pthread_self(),
        )
    }) {
        drop(unsafe { Box::from_raw(f) });
        return Err(e);
    }
    return Ok(());
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum Target<'a> {
    Document = 1,
    Window = 2,
    Screen = 3,
    Custom(&'a CStr),
}

impl Target<'_> {
    fn get(self) -> *const c_char {
        match self {
            Target::Document => EMSCRIPTEN_EVENT_TARGET_DOCUMENT,
            Target::Window => EMSCRIPTEN_EVENT_TARGET_WINDOW,
            Target::Screen => EMSCRIPTEN_EVENT_TARGET_SCREEN,
            Target::Custom(name) => name.as_ptr(),
        }
    }
}

impl<'a> From<&'a CStr> for Target<'a> {
    #[inline]
    fn from(value: &'a CStr) -> Self {
        Self::Custom(value)
    }
}

#[derive(Debug, Clone)]
pub struct FullscreenChangeEvent<'a> {
    pub is_fullscreen: bool,
    pub fullscreen_enabled: bool,
    pub node_name: &'a CStr,
    pub id: &'a CStr,
    pub element_width: c_int,
    pub element_height: c_int,
    pub screen_width: c_int,
    pub screen_height: c_int,
}

#[derive(Debug, Clone)]
pub struct UiEvent {
    pub detail: c_int,
    pub document_body_client_width: c_int,
    pub document_body_client_height: c_int,
    pub window_inner_width: c_int,
    pub window_inner_height: c_int,
    pub window_outer_width: c_int,
    pub window_outer_height: c_int,
    pub scroll_top: c_int,
    pub scroll_left: c_int,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[repr(i32)]
pub enum HtmlError {
    #[error("The requested operation cannot be completed now for web security reasons, and has been deferred for completion in the next event handler.")]
    Deferred = EMSCRIPTEN_RESULT_DEFERRED as c_int,
    #[error("The given operation is not supported by this browser or the target element. This value will be returned at the time the callback is registered if the operation is not supported.")]
    NotSupported = EMSCRIPTEN_RESULT_NOT_SUPPORTED as c_int,
    #[error("The requested operation could not be completed now for web security reasons. It failed because the user requested the operation not be deferred.")]
    FailedNotDeferred = EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED as c_int,
    #[error("The operation failed because the specified target element is invalid.")]
    InvalidTarget = EMSCRIPTEN_RESULT_INVALID_TARGET as c_int,
    #[error("The operation failed because the specified target element was not found.")]
    UnknownTarget = EMSCRIPTEN_RESULT_UNKNOWN_TARGET as c_int,
    #[error("The operation failed because an invalid parameter was passed to the function.")]
    InvalidParam = EMSCRIPTEN_RESULT_INVALID_PARAM as c_int,
    #[error("Generic failure result message, returned if no specific result is available.")]
    Failed = EMSCRIPTEN_RESULT_FAILED as c_int,
    #[error("The operation failed because no data is currently available.")]
    NoData = EMSCRIPTEN_RESULT_NO_DATA as c_int,
    #[error("Unknown response code '{0}'")]
    Unknown(c_int) = i32::MIN,
}

#[inline]
fn tri(res: c_int) -> Result<(), HtmlError> {
    const SUCCESS: c_int = EMSCRIPTEN_RESULT_SUCCESS as c_int;
    const DEFERRED: c_int = EMSCRIPTEN_RESULT_DEFERRED as c_int;

    return Err(match res {
        SUCCESS => return Ok(()),
        DEFERRED => HtmlError::Deferred,
        EMSCRIPTEN_RESULT_NOT_SUPPORTED => HtmlError::NotSupported,
        EMSCRIPTEN_RESULT_FAILED_NOT_DEFERRED => HtmlError::FailedNotDeferred,
        EMSCRIPTEN_RESULT_INVALID_TARGET => HtmlError::InvalidTarget,
        EMSCRIPTEN_RESULT_UNKNOWN_TARGET => HtmlError::UnknownTarget,
        EMSCRIPTEN_RESULT_INVALID_PARAM => HtmlError::InvalidParam,
        EMSCRIPTEN_RESULT_FAILED => HtmlError::Failed,
        EMSCRIPTEN_RESULT_NO_DATA => HtmlError::NoData,
        _ => HtmlError::Unknown(res),
    });
}
