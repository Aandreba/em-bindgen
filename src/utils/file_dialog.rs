use alloc::{
    alloc::Layout,
    borrow::Cow,
    ffi::{CString, NulError},
    fmt::{Debug, Display},
};
use core::time::Duration;
use libc::{c_char, c_void};
use libstd::{
    collections::{hash_map::Entry, HashMap},
    time::SystemTime,
};
use typed_arena::Arena;

#[derive(Debug, Clone, Default)]
pub struct FileDialog {
    file_name: Option<String>,
    filter: Vec<(String, Vec<String>)>,
}

impl FileDialog {
    pub fn set_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[impl Display]) -> Self {
        self.filter.push((
            name.into(),
            extensions.iter().map(|ext| format!(".{ext}")).collect(),
        ));
        self
    }
}

impl FileDialog {
    pub fn load_file(self) -> Option<FileHandle> {
        let accept = match self
            .filter
            .into_iter()
            .flat_map(|(_, extensions)| extensions)
            .reduce(|prev, curr| format!("{prev},{curr}"))
            .map(CString::new)
            .transpose()
        {
            Ok(Some(accept)) => Cow::Owned(accept),
            Ok(None) => Cow::Borrowed(c"*"),
            Err(e) => {
                log::error!("{e}");
                return None;
            }
        };

        unsafe {
            let file = sys::LoadFile(accept.as_ptr(), Some(memalloc));
            if file.contents.is_null() {
                return None;
            }

            return Some(FileHandle {
                last_modified: SystemTime::UNIX_EPOCH
                    + Duration::from_millis(file.last_modified_ms as u64),
                contents: Vec::from_raw_parts(file.contents, file.contents_len, file.contents_len),
            });
        }
    }

    pub fn save_file(self, contents: &[u8]) -> bool {
        macro_rules! tri {
            ($e:expr) => {
                match $e {
                    Ok(x) => x,
                    Err(e) => {
                        log::error!("{e}");
                        return false;
                    }
                }
            };
        }

        let c_str_arena = Arena::new();
        let extensions_arena = Arena::new();
        let accept_arena = Arena::new();

        let mut suggested_mime = self
            .file_name
            .as_deref()
            .map(mime_guess::from_path)
            .and_then(|mime| mime.first_raw());

        let types = tri!(self
            .filter
            .into_iter()
            .map(|(name, exts)| {
                let mut accept = HashMap::<_, Vec<*const c_char>>::new();
                c_str_arena.reserve_extend(exts.len());

                for ext in exts.into_iter() {
                    let mime = mime_guess::from_ext(&ext[1..])
                        .first_raw()
                        .or(suggested_mime);
                    suggested_mime = suggested_mime.or(mime);
                    let mime = mime.unwrap_or("application/octet-stream");

                    let ext = c_str_arena.alloc(CString::new(ext)?).as_ptr();
                    match accept.entry(mime) {
                        Entry::Occupied(entry) => entry.into_mut().push(ext),
                        Entry::Vacant(entry) => {
                            entry.insert(vec![ext]);
                        }
                    }
                }

                c_str_arena.reserve_extend(accept.len());
                let accept = accept_arena.alloc_extend(
                    accept
                        .into_iter()
                        .map(|(mime, exts)| {
                            let exts = extensions_arena.alloc(exts);
                            Ok::<_, NulError>(sys::Accept {
                                mime: c_str_arena.alloc(CString::new(mime)?).as_ptr(),
                                extensions: exts.as_ptr(),
                                extensions_len: exts.len(),
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                );

                return Ok::<_, NulError>(sys::FileType {
                    description: c_str_arena.alloc(CString::new(name)?).as_ptr(),
                    accept: accept.as_ptr(),
                    accept_len: accept.len(),
                });
            })
            .collect::<Result<Vec<_>, _>>());

        let suggested_mime = tri!(CString::new(
            suggested_mime.unwrap_or("application/octet-stream")
        ));
        let suggested_name = tri!(CString::new(self.file_name.unwrap_or_else(|| {
            SystemTime::UNIX_EPOCH
                .elapsed()
                .unwrap_or_else(|e| e.duration())
                .as_secs()
                .to_string()
        })));

        unsafe {
            return sys::SaveFile(
                contents.as_ptr(),
                contents.len(),
                suggested_name.as_ptr(),
                suggested_mime.as_ptr(),
                types.as_ptr(),
                types.len(),
            );
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileHandle {
    pub last_modified: SystemTime,
    pub contents: Vec<u8>,
}

mod sys {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]

    include!(concat!(env!("OUT_DIR"), "/file_dialog.rs"));
}

unsafe extern "C" fn memalloc(len: usize) -> *mut c_void {
    let Ok(layout) = Layout::array::<u8>(len) else {
        return std::ptr::null_mut();
    };
    return std::alloc::alloc(layout).cast();
}