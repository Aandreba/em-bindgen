use core::{cmp::Ordering, ffi::CStr, mem::ManuallyDrop, ptr::NonNull};
use docfg::docfg;
use std::ffi::{CString, NulError};
use val_sys::*;

#[derive(Debug)]
#[repr(transparent)]
pub struct JsValue {
    handle: NonNull<_EM_VAL>,
}

impl JsValue {
    pub const UNDEFINED: JsValue = JsValue {
        handle: unsafe { NonNull::new_unchecked(_EMVAL_UNDEFINED) },
    };

    pub const NULL: JsValue = JsValue {
        handle: unsafe { NonNull::new_unchecked(_EMVAL_NULL) },
    };

    pub const TRUE: JsValue = JsValue {
        handle: unsafe { NonNull::new_unchecked(_EMVAL_TRUE) },
    };

    pub const FALSE: JsValue = JsValue {
        handle: unsafe { NonNull::new_unchecked(_EMVAL_FALSE) },
    };

    pub fn from_str(s: impl Into<String>) -> Result<JsValue, NulError> {
        let cstr = CString::new(s.into())?;
        return unsafe { Ok(JsValue::from_utf8_c_str(&cstr)) };
    }

    pub unsafe fn from_utf8_c_str(s: &CStr) -> JsValue {
        let handle = unsafe { NonNull::new_unchecked(_emval_new_u8string(s.as_ptr())) };
        return JsValue { handle };
    }

    #[inline]
    pub fn from_bool(v: bool) -> JsValue {
        return match v {
            true => JsValue::TRUE,
            false => JsValue::FALSE,
        };
    }
}

impl JsValue {
    #[inline]
    pub fn as_handle(&self) -> EM_VAL {
        return self.handle.as_ptr();
    }

    #[inline]
    pub unsafe fn take_ownership(handle: EM_VAL) -> Self {
        return Self {
            handle: NonNull::new_unchecked(handle),
        };
    }

    /// Equivalent to `val::release_ownership`
    pub fn into_handle(self) -> EM_VAL {
        let this = ManuallyDrop::new(self);
        return this.as_handle();
    }

    #[inline]
    pub fn loose_eq(&self, other: &Self) -> bool {
        unsafe { _emval_equals(self.as_handle(), other.as_handle()) }
    }

    pub fn loose_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.loose_eq(other) {
            return Some(Ordering::Equal);
        } else if self < other {
            return Some(Ordering::Less);
        } else if self > other {
            return Some(Ordering::Greater);
        } else {
            return None;
        }
    }

    #[docfg(feature = "asyncify")]
    pub fn block_on(&self) -> JsValue {
        JsValue {
            handle: unsafe { NonNull::new_unchecked(_emval_await(self.as_handle())) },
        }
    }

    #[inline]
    fn uses_ref_count(&self) -> bool {
        return self.handle.as_ptr() > _EMVAL_LAST_RESERVED_HANDLE;
    }
}

impl PartialEq for JsValue {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        unsafe { _emval_strictly_equals(self.as_handle(), other.as_handle()) }
    }
}

impl PartialOrd for JsValue {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        if self == other {
            return Some(Ordering::Equal);
        } else if self < other {
            return Some(Ordering::Less);
        } else if self > other {
            return Some(Ordering::Greater);
        } else {
            return None;
        }
    }

    #[inline]
    fn lt(&self, other: &Self) -> bool {
        unsafe { _emval_less_than(self.as_handle(), other.as_handle()) }
    }

    #[inline]
    fn gt(&self, other: &Self) -> bool {
        unsafe { _emval_greater_than(self.as_handle(), other.as_handle()) }
    }
}

impl Clone for JsValue {
    #[inline]
    fn clone(&self) -> Self {
        if self.uses_ref_count() {
            unsafe { _emval_incref(self.as_handle()) }
        }
        return Self {
            handle: self.handle,
        };
    }
}

impl Drop for JsValue {
    #[inline]
    fn drop(&mut self) {
        if self.uses_ref_count() {
            unsafe { _emval_decref(self.as_handle()) }
        }
    }
}

mod val_sys {
    #![allow(non_camel_case_types)]
    #![allow(unused)]

    use std::ffi::*;

    macro_rules! opaque {
        ($($name:ident),+ $(,)?) => {
        	$(
	        	concat_idents::concat_idents!(opaque_name = _, $name {
		            pub type $name = *mut opaque_name;

		            #[repr(C)]
		            #[derive(Debug, Copy, Clone)]
		            #[doc(hidden)]
		            pub struct opaque_name {
		                _unused: [u8; 0],
		            }
	        	});
    		)+
        };
    }

    type EM_GENERIC_WIRE_TYPE = f64;
    type EM_VAR_ARGS = *const c_void;

    opaque! {
        EM_VAL,
        EM_DESTRUCTORS,
        EM_METHOD_CALLER,
    }

    pub const _EMVAL_UNDEFINED: EM_VAL = 2 as EM_VAL;
    pub const _EMVAL_NULL: EM_VAL = 4 as EM_VAL;
    pub const _EMVAL_TRUE: EM_VAL = 6 as EM_VAL;
    pub const _EMVAL_FALSE: EM_VAL = 8 as EM_VAL;
    pub const _EMVAL_LAST_RESERVED_HANDLE: EM_VAL = 8 as EM_VAL;

    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn _emval_incref(value: EM_VAL);
        pub fn _emval_decref(value: EM_VAL);

        pub fn _emval_new_array() -> EM_VAL;
        pub fn _emval_new_array_from_memory_view(mv: EM_VAL) -> EM_VAL;
        pub fn _emval_new_object() -> EM_VAL;
        pub fn _emval_new_cstring(s: *const c_char) -> EM_VAL;
        pub fn _emval_new_u8string(s: *const c_char) -> EM_VAL;

        pub fn _emval_get_global(name: *const c_char) -> EM_VAL;
        pub fn _emval_get_module_property(name: *const c_char) -> EM_VAL;
        pub fn _emval_get_property(object: EM_VAL, key: EM_VAL) -> EM_VAL;
        pub fn _emval_set_property(object: EM_VAL, key: EM_VAL, value: EM_VAL);

        pub fn _emval_equals(first: EM_VAL, second: EM_VAL) -> bool;
        pub fn _emval_strictly_equals(first: EM_VAL, second: EM_VAL) -> bool;
        pub fn _emval_greater_than(first: EM_VAL, second: EM_VAL) -> bool;
        pub fn _emval_less_than(first: EM_VAL, second: EM_VAL) -> bool;
        pub fn _emval_not(object: EM_VAL);

        pub fn _emval_throw(object: EM_VAL) -> !;
        pub fn _emval_await(promise: EM_VAL) -> EM_VAL;
    }
}
