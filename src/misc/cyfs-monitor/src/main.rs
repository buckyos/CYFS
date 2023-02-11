use std::path::Path;
use std::process::ExitCode;
use crate::monitor::MontiorConfig;
use cyfs_base::*;
use clap::{App, Arg};

mod case;
mod def;
mod monitor;
mod report;

#[macro_use]
extern crate log;

const SERVICE_NAME: &str = "monitor";

fn get_config(config_path: &Path) -> BuckyResult<MontiorConfig> {
    Ok(toml::from_slice(&std::fs::read(config_path)?).map_err(|e| {
        error!("parse config err {}", e);
        BuckyError::from(BuckyErrorCode::InvalidFormat)
    })?)
}

#[async_std::main]
async fn main() -> ExitCode {
    let matches = App::new("cyfs-monitor")
        .version(cyfs_base::get_version())
        .about("monitor cyfs system")
        .arg(Arg::with_name("cases").takes_value(true).multiple(true))
        .arg(Arg::with_name("args").last(true))
        .get_matches();
    let cases = matches.values_of("cases").map(|i|{i.collect()}).unwrap_or(vec![]);
    let run_once = cases.len() > 0;
    cyfs_debug::CyfsLoggerBuilder::new_app(SERVICE_NAME)
        .level("info")
        .file(!run_once)
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs", SERVICE_NAME)
        .exit_on_panic(true)
        .build()
        .start();

    let config_path = std::env::current_exe().unwrap().parent().unwrap().join("monitor.toml");
    let monitor_config = get_config(&config_path).unwrap_or(MontiorConfig::default());

    let monitor = monitor::Monitor::new(monitor_config)
        .await
        .map_err(|e| {
            error!("init monitor error! {}", e);
            std::process::exit(-1);
        })
        .unwrap();

    let ret = monitor.run(cases).await;
    if ret.is_ok() {
        ExitCode::from(0)
    } else {
        ExitCode::from(ret.unwrap_err().code().as_u8())
    }
}

#[cfg(test)]
mod test {
    use crate::def::MonitorErrorInfo;
    use crate::monitor::*;
    use crate::report::*;
    use cyfs_base::*;

    #[async_std::test]
    async fn test_report() {
        cyfs_base::init_log("test-monitor", Some("debug"));

        let config = MontiorConfig {
            dingtalk_url: Some("https://oapi.dingtalk.com/robot/send?access_token=28788f9229a09bfe8b33e678d4447a2d2d80a334a594e1c942329cab8581f422".to_owned()),
        };

        let report_manager = BugReportManager::new(&config);
        let info = MonitorErrorInfo {
            service: "test".to_owned(),
            error: BuckyError::new(BuckyErrorCode::Failed, "test message"),
        };

        report_manager.report(&info).await;
    }
}
