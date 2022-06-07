use log::*;
use crate::combine_logger::{str_to_logfilter_level};
use std::os::raw::c_char;
use std::ffi::CString;
use once_cell::sync::OnceCell;

pub type LogCallback = extern "C" fn(*const c_char);

pub struct IosLogger {
    level: LevelFilter,
    callback: LogCallback
}

impl IosLogger {
    pub fn new(level_str: &str, callback: LogCallback) -> Self {
        Self{
            level: str_to_logfilter_level(level_str),
            callback
        }
    }
}


impl Log for IosLogger {
    fn enabled(&self, meta: &Metadata) -> bool {
        meta.level() <= self.level
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return
        }
        let msg = format!("{}", record.args());
        (self.callback)(CString::new(msg.as_str()).unwrap().as_ptr());
    }
    fn flush(&self) {}
}