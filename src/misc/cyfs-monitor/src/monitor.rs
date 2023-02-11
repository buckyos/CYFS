use std::collections::HashMap;
use crate::case::*;
use crate::def::*;
use crate::report::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Timelike;
use cyfs_base_meta::SavedMetaObject;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_util::SNDirParser;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontiorConfig {
    #[serde(default = "default_url")]
    pub dingtalk_url: Option<String>,
    pub monitor_name: Option<String>
}

fn default_url() -> Option<String> {
    option_env!("DINGTOKEN").map(|token| {
        format!("https://oapi.dingtalk.com/robot/send?access_token={}", token)
    })
}

impl Default for MontiorConfig {
    fn default() -> Self {
        Self {
            dingtalk_url: default_url(),
            monitor_name: None,
        }
    }
}

#[derive(Clone)]
struct MonitorCase {
    runner: Arc<Box<dyn MonitorRunner>>,
    timeout_secs: u64,
    org_interval_minutes: u64,
    interval_minutes: u64,
    remain_minutes: u64,
    fail_times: u8,
    report_after_fail_times: u8
}

impl MonitorCase {
    fn countdown(&mut self) -> bool {
        if self.remain_minutes > 0 {
            self.remain_minutes = self.remain_minutes - 1;
            false
        } else {
            self.remain_minutes = self.interval_minutes - 1;
            true
        }
    }

    fn set_interval(&mut self, new_interval: u64) {
        info!("set case {} interval from {} to {} minutes", self.runner.name(), self.interval_minutes, new_interval);
        self.interval_minutes = new_interval;
        self.remain_minutes = self.interval_minutes - 1;
    }

    fn failed(&mut self) -> bool {
        let mut new_interval = self.interval_minutes / 2;
        if new_interval * 60 <= self.timeout_secs {
            new_interval = self.timeout_secs / 60 + 1;
        }
        self.set_interval(new_interval);

        if self.fail_times >= self.report_after_fail_times {
            self.fail_times = 1;
        } else {
            self.fail_times = self.fail_times + 1;
        }

        info!("case {} run err, before report {}/{}",
                    self.runner.name(),
                    self.fail_times,
                    self.report_after_fail_times);

        self.fail_times >= self.report_after_fail_times
    }

    fn success(&mut self) {
        if self.fail_times > 0 {
            info!("case {} success after {} fails",
                    self.runner.name(),
                    self.fail_times);
            self.fail_times = 0;
            self.set_interval(self.org_interval_minutes);
        }
    }

    fn name(&self) -> &str {
        self.runner.name()
    }
}

pub struct Monitor {
    monitor_list: HashMap<String, MonitorCase>,
    bug_reporter: Arc<BugReportManager>,
    name: String
}

async fn get_sn_devices_from_meta() -> BuckyResult<Vec<(DeviceId, Device)>> {
    let meta_client = MetaClient::new_target(MetaMinerTarget::default());
    let (info, _) = meta_client.get_name(CYFS_SN_NAME).await?.ok_or(BuckyError::from(BuckyErrorCode::NotFound))?;
    if let NameLink::ObjectLink(id) = info.record.link {
        let data = meta_client.get_desc(&id).await?;
        if let SavedMetaObject::Data(data) = data {
            Ok(SNDirParser::parse(Some(&data.id), &data.data)?)
        } else {
            warn!("get sn list name from meta but not support");
            Err(BuckyError::from(BuckyErrorCode::NotMatch))
        }

    } else {
        warn!("get sn list name from meta but not support");
        Err(BuckyError::from(BuckyErrorCode::NotSupport))
    }
}

async fn get_sn_devices() -> Vec<Device> {
    get_sn_devices_from_meta().await.unwrap_or_else(|e| {
        warn!("get sn list from meta err {}, use built-in sn list", e);
        cyfs_util::get_sn_desc().clone()
    }).iter().map(|(_, device)| {
        device.clone()
    }).collect()
}

impl Monitor {
    pub async fn new(config: MontiorConfig) -> BuckyResult<Self> {
        info!("use config {:?}", &config);
        let bug_reporter = BugReportManager::new(&config);
        let mut monitor = Self {
            monitor_list: HashMap::new(),
            bug_reporter: Arc::new(bug_reporter),
            name: config.monitor_name.as_ref().map(|name|format!("monitor-{}", &name)).unwrap_or("monitor".to_owned())
        };

        // 添加sn测试用例
        let sn_devices = get_sn_devices().await;
        for sn_device in sn_devices {
            info!("add sn online monitor {}", sn_device.desc().calculate_id());
            monitor.add_case(SNOnlineMonitor::new(sn_device), 5, 20, 3);
        }

        //添加测试用例
        // monitor.add_case(ExampleCaseMonitor::new(true), 3, 10, 3);
        monitor.add_case(MetaChainReadMonitor::new(), 5, 20, 2);
        monitor.add_case(MetaChainWriteMonitor::new(), 60, 60, 1);
        monitor.add_case(CyfsRepoMonitor::new(), 60, 60, 1);

        Ok(monitor)
    }

    fn add_case<C: 'static + MonitorRunner>(&mut self, case: C, interval_mintue: u64, timeout_sec: u64, report_after_fail: u8) {
        let case = MonitorCase {
            runner: Arc::new(Box::new(case)),
            timeout_secs: timeout_sec,
            org_interval_minutes: interval_mintue,
            interval_minutes: interval_mintue,
            remain_minutes: 0,
            fail_times: 0,
            report_after_fail_times: report_after_fail
        };
        self.monitor_list.insert(case.runner.name().to_owned(), case);
    }

    pub async fn run(mut self, cases: Vec<&str>) ->BuckyResult<()> {
        info!("start monitor");
        loop {
            // 每天本地时间17：30，上报一条自己正在正常工作的消息
            let time = chrono::Local::now();
            if time.hour() == 17 && time.minute() == 30 {
                let _ = self.bug_reporter.report(&MonitorErrorInfo {
                    service: self.name.clone(),
                    case: "Alive Report".to_owned(),
                    error: BuckyError::new(BuckyErrorCode::Ok, "CYFS Monitor Running"),
                    at_all: false
                }).await;
            }
            let ret = self.run_once(cases.as_slice()).await;
            if !cases.is_empty() {
                return ret;
            }

            async_std::task::sleep(std::time::Duration::from_secs(60 * 1)).await;
        }
    }

    pub async fn run_once(&mut self, cases: &[&str]) -> BuckyResult<()> {
        let mut run_cases = vec![];
        for (_, item) in self.monitor_list.iter_mut() {
            info!("check case {}, left {}", item.runner.name(), item.remain_minutes);
            if !cases.is_empty() && cases.contains(&item.runner.name()) {
                info!("run case {} once", item.runner.name());
                run_cases.push((item.name().to_owned(), true))
            } else if cases.is_empty(){
                if item.countdown() {
                    info!("case {} will run now", item.runner.name());
                    run_cases.push((item.name().to_owned(), false));
                }
            }
        }

        let mut run_futures = vec![];
        for (case_name, once) in &run_cases {
            let case = self.monitor_list.get(case_name).unwrap();
            let runner = case.runner.clone();
            let ret = async_std::future::timeout(
                std::time::Duration::from_secs(case.timeout_secs + if *once {0} else {1} as u64),
                async move {
                    let name = runner.name();
                    let tick = std::time::Instant::now();
                    debug!("will run monitor case: {}", name);

                    let ret = runner.run_once(*once).await;
                    let duration = tick.elapsed().as_secs();
                    if ret.is_err() {
                        let e = ret.unwrap_err();
                        error!(
                            "run monitor case error! name={}, duration={}s, {}",
                            name, duration, e
                        );
                        Err(e)
                    } else {
                        info!(
                            "run monitor case success! name={}, duration={}s",
                            name, duration
                        );
                        Ok(())
                    }
                },
            );
            run_futures.push(ret);
        }
        let rets = futures::future::join_all(run_futures).await;
        let mut return_status = Ok(());
        // 有一个case出错，就返回最后一个出错的值
        for (ret, (case_name, _)) in rets.iter().zip(run_cases.iter()) {
            let ret = match ret {
                Ok(ret) => {
                    ret.clone()
                }
                Err(_) => {
                    Err(BuckyError::new(BuckyErrorCode::Timeout, format!("run case {} timeout", case_name)))
                }
            };
            let real_case = self.monitor_list.get_mut(case_name).unwrap();
            if ret.is_err() {
                return_status = ret.clone();

                if real_case.failed() {
                    let info = MonitorErrorInfo {
                        service: self.name.clone(),
                        error: ret.unwrap_err(),
                        at_all: true,
                        case: real_case.runner.name().to_owned()
                    };
                    let _ = self.bug_reporter.report(&info).await;
                }
            } else {
                real_case.success()
            }
        }

        return_status
    }
}
