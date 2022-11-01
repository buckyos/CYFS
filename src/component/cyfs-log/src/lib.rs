mod bug_report;
mod debug_config;
mod log;
mod log_util;
mod panic;

#[cfg(feature = "http_report")]
mod http_target;

pub use crate::log::*;
pub use debug_config::*;
pub use log_util::*;
pub use panic::*;

#[cfg(feature = "http_report")]
pub use http_target::*;
/*
static VERSION: once_cell::sync::OnceCell<&'static str> = once_cell::sync::OnceCell::new();

fn version() -> &'static str {
    VERSION.get().unwrap_or(&"version not inited")
}

pub fn set_version(version: &'static str) {
    let _ = VERSION.set(version);
}*/

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_log() {
        CyfsLoggerBuilder::new_app("test-cyfs-debug")
            .level("trace")
            .console("trace")
            .enable_bdt(Some("warn"), Some("warn"))
            .build()
            .unwrap()
            .start();
    }
}

#[macro_use]
extern crate log as _log;
