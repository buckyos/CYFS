use super::panic::*;
use crate::bug_report::*;
use cyfs_base::*;
use cyfs_util::*;

use backtrace::Backtrace;
use chrono::offset::Local;
use chrono::DateTime;
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;

// 触发了panic
pub type FnOnPanic = dyn EventListenerAsyncRoutine<CyfsPanicInfo, ()>;
type OnPanicEventManager = SyncEventManagerSync<CyfsPanicInfo, ()>;

pub trait BugReportHandler: Send + Sync {
    fn notify(
        &self,
        product_name: &str,
        service_name: &str,
        panic_info: &CyfsPanicInfo,
    ) -> BuckyResult<()>;
}

struct PanicManagerImpl {
    product_name: String,
    service_name: String,

    log_to_file: bool,
    log_dir: PathBuf,

    exit_on_panic: bool,

    on_panic: OnPanicEventManager,

    // reporters
    reporter: Option<Box<dyn BugReportHandler>>,
}

impl PanicManagerImpl {
    pub fn new(builder: PanicBuilder) -> Self {
        let reporter = if !builder.disable_bug_report {
            let mut reporter = BugReportManager::new(builder.bug_reporter);
            if reporter.is_empty() {
                reporter.load_from_config();
            }

            Some(Box::new(reporter) as Box<dyn BugReportHandler>)
        } else {
            None
        };

        Self {
            product_name: builder.product_name,
            service_name: builder.service_name,
            log_to_file: builder.log_to_file,
            log_dir: builder.log_dir,
            exit_on_panic: builder.exit_on_panic,

            on_panic: OnPanicEventManager::new(),
            reporter,
        }
    }
}

#[derive(Clone)]
pub struct PanicManager(Arc<PanicManagerImpl>);

impl PanicManager {
    pub fn new(builder: PanicBuilder) -> Self {
        Self(Arc::new(PanicManagerImpl::new(builder)))
    }

    pub fn start(&self) {
        let this = self.clone();
        panic::set_hook(Box::new(move |info| {
            let backtrace = Backtrace::new();
            let pinfo = CyfsPanicInfo::new(backtrace, info);
            let this = this.clone();

            let _ = std::thread::spawn(move || {
                this.on_panic(pinfo);
            }).join();
        }));
    }

    pub fn event(&self) -> OnPanicEventManager {
        self.0.on_panic.clone()
    }

    fn on_panic(&self, info: CyfsPanicInfo) {
        if self.0.log_to_file {
            self.log_to_file(&info);
        }

        if let Some(reporter) = &self.0.reporter {
            info!("will report panic......");
            let _ = reporter.notify(&self.0.product_name, &self.0.service_name, &info);
        }

        // 触发事件
        let _ = self.0.on_panic.emit(&info);

        if self.0.exit_on_panic {
            crate::CyfsLogger::flush();

            error!("process will exit on panic......");
            std::thread::sleep(std::time::Duration::from_secs(3));
            error!("process exit on panic......");
            std::process::exit(-1);
        }
    }

    fn log_to_file(&self, info: &CyfsPanicInfo) {
        if let Err(e) = std::fs::create_dir_all(&self.0.log_dir) {
            error!(
                "create panic dir failed! dir={}, {}",
                self.0.log_dir.display(),
                e
            );
            return;
        }

        let file_name = format!("{}_panic_{}.log", self.0.service_name, info.hash);

        let now = std::time::SystemTime::now();
        let datetime: DateTime<Local> = now.into();
        let now = datetime.format("%Y_%m_%d %H:%M:%S.%f");

        let content;
        #[cfg(debug_assertions)]
        {
            content = format!("{}\n{}", now, info.msg_with_symbol);
        }

        #[cfg(not(debug_assertions))]
        {
            content = format!("{}\n{}", now, info.msg);
        }

        let file_path = self.0.log_dir.join(file_name);
        if let Err(e) = std::fs::write(&file_path, content) {
            error!("write panic log failed! dir={}, {}", file_path.display(), e);
        }
    }
}

pub struct PanicBuilder {
    product_name: String,
    service_name: String,

    log_to_file: bool,
    log_dir: PathBuf,

    disable_bug_report: bool,
    bug_reporter: Vec<Box<dyn BugReportHandler>>,

    // Whether to end the process after PANIC
    exit_on_panic: bool,
}

impl PanicBuilder {
    pub fn new(product_name: &str, service_name: &str) -> Self {
        assert!(!product_name.is_empty());
        assert!(!service_name.is_empty());

        let mut root = get_cyfs_root_path();
        root.push("log/panic");
        root.push(product_name);

        Self {
            product_name: product_name.to_owned(),
            service_name: service_name.to_owned(),
            log_to_file: true,
            log_dir: root,
            disable_bug_report: false,
            bug_reporter: vec![],
            exit_on_panic: false,
        }
    }

    // panic信息是否输出到日志文件，默认输出
    pub fn log_to_file(mut self, enable: bool) -> Self {
        self.log_to_file = enable;
        self
    }

    // panic输出到的日志目录，默认是{cyfs_root}/log/panic/{product_name}/
    pub fn log_dir(mut self, log_dir: impl Into<PathBuf>) -> Self {
        self.log_dir = log_dir.into();
        self
    }

    pub fn bug_report(mut self, handler: Box<dyn BugReportHandler>) -> Self {
        self.bug_reporter.push(handler);
        self
    }

    // use default http bug_report impl for
    pub fn http_bug_report(mut self, url: &str) -> Self {
        let handler = HttpBugReporter::new(url);
        self.bug_reporter.push(Box::new(handler));
        self
    }

    // use dingtalk bug_report
    pub fn dingtalk_bug_report(mut self, dingtalk_url: &str) -> Self {
        let handler = DingtalkNotifier::new(dingtalk_url);
        self.bug_reporter.push(Box::new(handler));
        self
    }

    pub fn disable_bug_report(mut self) -> Self {
        self.disable_bug_report = true;
        self
    }

    // panic后是否结束进程，默认不结束
    pub fn exit_on_panic(mut self, exit: bool) -> Self {
        self.exit_on_panic = exit;
        self
    }

    pub fn build(self) -> PanicManager {
        PanicManager::new(self)
    }
}
