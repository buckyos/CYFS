use panic::PanicInfo;

use backtrace::{Backtrace, BacktraceFrame};
use sha2::Digest;
use std::panic;
use std::thread;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CyfsPanicInfo {
    pub msg: String,
    pub msg_with_symbol: String,
    pub hash: String,
}

impl CyfsPanicInfo {
    pub fn new(backtrace: Backtrace, info: &PanicInfo) -> Self {
        let backtrace_msg = Self::format_backtrace(&backtrace);
        let msg = Self::format_info(info, &backtrace_msg);

        let backtrace_msg = Self::format_backtrace_with_symbol(&backtrace);
        let msg_with_symbol = Self::format_info(info, &backtrace_msg);

        let hash = Self::calc_hash(&backtrace);
        let ret = Self {
            msg,
            msg_with_symbol,
            hash,
        };

        warn!("{}", ret.msg);
        warn!("{}", ret.msg_with_symbol);
        ret
    }

    fn format_info(info: &PanicInfo, backtrace: &str) -> String {
        let thread = thread::current();
        let thread = thread.name().unwrap_or("unnamed");

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &**s,
                None => "Box<Any>",
            },
        };

        let msg = match info.location() {
            Some(location) => {
                format!(
                    "thread '{}' panicked at '{}': {}:{}\n{}",
                    thread,
                    msg,
                    location.file(),
                    location.line(),
                    backtrace,
                )
            }
            None => {
                format!(
                    "thread '{}' panicked at '{}'\n{}",
                    thread,
                    msg,
                    backtrace.clone()
                )
            }
        };

        msg
    }

    fn format_backtrace_with_symbol(backtrace: &Backtrace) -> String {
        format!("{:?}", backtrace)
    }

    fn format_backtrace(backtrace: &Backtrace) -> String {
        let frames: Vec<BacktraceFrame> = backtrace.clone().into();
        let mut values = Vec::new();
        for (i, frame) in frames.into_iter().enumerate() {
            if let Some(mod_addr) = frame.module_base_address() {
                let offset = frame.symbol_address() as isize - mod_addr as isize;
                values.push(format!("{}: {:#018x} {:#018p}", i, offset, mod_addr));
            } else {
                values.push(format!("{}: {:#018p}", i, frame.symbol_address()));
            }
        }

        values.join("\n")
    }

    fn calc_hash(backtrace: &Backtrace) -> String {
        let mut sha256 = sha2::Sha256::new();

        let frames: Vec<BacktraceFrame> = backtrace.clone().into();
        let mut values = Vec::new();
        for (i, frame) in frames.into_iter().enumerate() {
            if let Some(mod_addr) = frame.module_base_address() {
                let offset = frame.symbol_address() as isize - mod_addr as isize;
                values.push(format!("{}:{}", i, offset));
            } else {
                values.push(format!("{}:{:p}", i, frame.symbol_address()));
            }
        }

        let all = values.join("\n");

        sha256.input(all);
        let ret = sha256.result();
        let hash = hex::encode(ret);

        // 只截取前32个字节
        let hash = hash[..32].to_owned();

        info!("stack_hash=\n{}", hash);

        hash
    }
}
