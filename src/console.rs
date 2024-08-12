use crate::{chrono::Local, sys::*};
use alloc::ffi::CString;
use core::ffi::CStr;
use log::{Level, LevelFilter, Log, SetLoggerError};

#[inline]
pub fn init() -> Result<(), SetLoggerError> {
    init_with_level(match cfg!(debug_assertions) {
        true => LevelFilter::Debug,
        false => LevelFilter::Info,
    })
}

#[inline]
pub fn init_with_level(level: LevelFilter) -> Result<(), SetLoggerError> {
    let logger = EmscriptenLogger { level };
    log::set_logger(Box::leak(Box::new(logger)))?;
    log::set_max_level(level);
    return Ok(());
}

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

struct EmscriptenLogger {
    level: LevelFilter,
}

impl Log for EmscriptenLogger {
    #[inline]
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.level >= metadata.level()
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let target = match record.target().is_empty() {
            true => record.target(),
            false => record
                .module_path()
                .or(record.module_path_static())
                .or(record.file())
                .or(record.file_static())
                .unwrap_or_default(),
        };

        let Ok(msg) = CString::new(format!(
            "{} [{}] {}",
            Local::now().to_rfc3339(),
            target,
            record.args()
        )) else {
            console_warn(c"Invalid logging message");
            return;
        };

        match record.level() {
            Level::Error => console_error(&msg),
            Level::Warn => console_warn(&msg),
            Level::Info | Level::Debug | Level::Trace => console_log(&msg),
        }
    }

    #[inline(always)]
    fn flush(&self) {}
}
