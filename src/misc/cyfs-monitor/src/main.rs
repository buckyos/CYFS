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

fn load(config_path: &Path, key: &str) -> BuckyResult<toml::Value> {
    let contents = std::fs::read_to_string(config_path)
        .map_err(|e| {
            let msg = format!(
                "load monitor config failed! file={}, err={}",
                config_path.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

    let cfg_node: toml::Value = toml::from_str(&contents).map_err(|e| {
        let msg = format!(
            "parse monitor config failed! file={}, content={}, err={}",
            config_path.display(),
            contents,
            e
        );
        error!("{}", msg);

        BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
    })?;

    let node = cfg_node.as_table().ok_or_else(|| {
        let msg = format!(
            "invalid config format! file={}, content={}",
            config_path.display(),
            contents,
        );
        error!("{}", msg);

        BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
    })?;

    let node = node.get(key).ok_or_else(|| {
        println!("config node not found! key={}", key);
        BuckyError::from(BuckyErrorCode::NotFound)
    })?;

    Ok(node.clone())
}

async fn load_config() -> BuckyResult<MontiorConfig> {
    let mut monitor_config = MontiorConfig::default();

    // 加载服务配置
    if let Ok(config) = load(Path::new("./monitor.toml"), "config") {
        let node = config.as_table().ok_or_else(|| {
            let msg = format!(
                "invalid log config format! content={}",
                config,
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        }).unwrap();

        for (k, v) in node {
            match k.as_str() {
                "dingtalk_url" => {
                    monitor_config.dingtalk_url = Some(v.as_str().unwrap().to_string());
                }
                _ => {
                    warn!("unknown monitor.config field: {}", k.as_str());
                }
            }
        }

        info!("monitor_config: {:?}", monitor_config);
    } else {
        warn!("cannot load monitor config file, use default config {:?}", monitor_config);
    };

    Ok(monitor_config)
}

#[async_std::main]
async fn main() -> ExitCode {
    let matches = App::new("cyfs-monitor")
        .version(cyfs_base::get_version())
        .about("monitor cyfs system")
        .arg(Arg::with_name("cases").takes_value(true).multiple(true))
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

    let monitor_config = load_config()
        .await
        .map_err(|_e| {
            std::process::exit(-1);
        })
        .unwrap();

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
