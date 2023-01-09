use std::collections::HashMap;
use crate::case::*;
use crate::def::*;
use crate::report::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use chrono::Timelike;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MontiorConfig {
    pub dingtalk_url: Option<String>,
}

impl Default for MontiorConfig {
    fn default() -> Self {
        let url = option_env!("DINGTOKEN").map(|token| {
            format!("https://oapi.dingtalk.com/robot/send?access_token={}", token)
        });
        Self { dingtalk_url: url, }
    }
}

#[derive(Clone)]
struct MonitorCase {
    runner: Arc<Box<dyn MonitorRunner>>,
    timeout_secs: u64,
    interval_minutes: u64,
    remain_minutes: u64,
    fail_times: u8,
    report_after_fail_times: u8
}

impl MonitorCase {
    fn countdown(&mut self) -> bool {
        if self.remain_minutes == 0 {
            self.remain_minutes = self.interval_minutes - 1;
        } else {
            self.remain_minutes = self.remain_minutes - 1;
        }

        self.remain_minutes == 0
    }

    fn failed(&mut self) -> bool {
        let mut new_interval = self.interval_minutes / 2;
        if new_interval * 60 <= self.timeout_secs {
            new_interval = self.timeout_secs / 60 + 1;
        }
        self.interval_minutes = new_interval;

        if self.fail_times == self.report_after_fail_times {
            self.fail_times = 1;
        } else {
            self.fail_times = self.fail_times + 1;
        }

        info!("case {} set new interval_minutes to {} because run err, before report {}/{}",
                    self.runner.name(),
                    self.interval_minutes,
                    self.fail_times,
                    self.report_after_fail_times);

        self.fail_times == self.report_after_fail_times
    }

    fn success(&mut self) {
        if self.fail_times > 0 {
            info!("case {} success after {} fails",
                    self.runner.name(),
                    self.fail_times);
            self.fail_times = 0;
        }
    }
}

pub struct Monitor {
    monitor_list: HashMap<String, MonitorCase>,

    bug_reporter: Arc<BugReportManager>,
}

impl Monitor {
    pub async fn new(config: MontiorConfig) -> BuckyResult<Self> {
        let bug_reporter = BugReportManager::new(&config);
        let mut monitor = Self {
            monitor_list: HashMap::new(),
            bug_reporter: Arc::new(bug_reporter),
        };

        //添加测试用例
        // monitor.add_case(ExampleCaseMonitor::new(true), 3, 10, 3);
        monitor.add_case(MetaChainReadMonitor::new(), 5, 20, 2);
        monitor.add_case(MetaChainWriteMonitor::new(), 60, 60, 1);
        monitor.add_case(SNOnlineMonitor::new(), 5, 10, 3);
        monitor.add_case(CyfsRepoMonitor::new(), 60, 60, 1);

        Ok(monitor)
    }

    fn add_case<C: 'static + MonitorRunner>(&mut self, case: C, interval_mintue: u64, timeout_sec: u64, report_after_fail: u8) {
        let case = MonitorCase {
            runner: Arc::new(Box::new(case)),
            timeout_secs: timeout_sec,
            interval_minutes: interval_mintue,
            remain_minutes: 0,
            fail_times: 0,
            report_after_fail_times: report_after_fail
        };
        self.monitor_list.insert(case.runner.name().to_owned(), case);
    }

    pub async fn run(mut self, cases: Vec<&str>) ->BuckyResult<()> {
        loop {
            // 每天本地时间17：30，上报一条自己正在正常工作的消息
            let time = chrono::Local::now();
            if time.hour() == 17 && time.minute() == 30 {
                let _ = self.bug_reporter.report(&MonitorErrorInfo {
                    service: "monitor".to_string(),
                    error: BuckyError::new(BuckyErrorCode::Ok, "CYFS Monitor Running")
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
        for (_, item) in &mut self.monitor_list {
            if !cases.is_empty() && cases.contains(&item.runner.name()) {
                info!("run case {} once", item.runner.name());
                run_cases.push((item.clone(), true))
            } else if cases.is_empty(){
                if item.countdown() {
                    info!("case {} will run now", item.runner.name());
                    run_cases.push((item.clone(), false));
                }
            }
        }
        let mut run_futures = vec![];
        for (case, once) in &run_cases {
            let case = case.clone();
            let ret = async_std::future::timeout(
                std::time::Duration::from_secs(case.timeout_secs + if *once {0} else {1} as u64),
                async move {
                    let name = case.runner.name();
                    let tick = std::time::Instant::now();
                    debug!("will run monitor case: {}", name);

                    let ret = case.runner.run_once(*once).await;
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
        for (ret, (case, _)) in rets.iter().zip(run_cases.iter()) {
            let ret = match ret {
                Ok(ret) => {
                    ret.clone()
                }
                Err(_) => {
                    Err(BuckyError::new(BuckyErrorCode::Timeout, format!("run case timeout after {} secs", case.timeout_secs)))
                }
            };
            let real_case = self.monitor_list.get_mut(case.runner.name()).unwrap();
            if ret.is_err() {
                return_status = ret.clone();

                if real_case.failed() {
                    let info = MonitorErrorInfo {
                        service: real_case.runner.name().to_owned(),
                        error: ret.unwrap_err(),
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
