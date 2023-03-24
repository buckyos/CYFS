mod bug_report;
mod debug_config;
mod log;
mod log_util;
mod panic;
mod dump;

#[cfg(feature = "http_report")]
mod http_target;

#[macro_use]
mod check;

pub use crate::log::*;
pub use check::*;
pub use debug_config::*;
pub use log_util::*;
pub use panic::*;
pub use dump::*;
pub use bug_report::PanicReportRequest;

#[cfg(feature = "http_report")]
pub use http_target::*;


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

        info!("create minidump file");
        let helper = dump::DumpHelper::get_instance();
        helper.dump();
    }
}

#[macro_use]
extern crate log as _log;
