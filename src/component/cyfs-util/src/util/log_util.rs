use super::path_util::{get_app_log_dir, get_log_dir};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use flexi_logger::{
    opt_format, Cleanup, Criterion, DeferredNow, Duplicate, Level, Logger, Naming, Record,
};
use std::path::{Path, PathBuf};

fn cyfs_default_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} [{}] {}",
        now.now().format("%Y-%m-%d %H:%M:%S%.3f"),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        //record.file().unwrap_or("<unnamed>"),
        //record.line().unwrap_or(0),
        &record.args()
    )
}

#[cfg(feature = "colors")]
pub fn cyfs_colored_default_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> Result<(), std::io::Error> {
    let level = record.level();
    write!(
        w,
        "[{}] {} [{}] {}",
        style(level, now.now().format("%Y-%m-%d %H:%M:%S%.3f")),
        style(level, record.level()),
        record.module_path().unwrap_or("<unnamed>"),
        //record.file().unwrap_or("<unnamed>"),
        //record.line().unwrap_or(0),
        style(level, &record.args())
    )
}

fn str_to_duplevel(level: &str) -> Duplicate {
    match level {
        "none" => Duplicate::None,
        "trace" => Duplicate::Trace,
        "debug" => Duplicate::Debug,
        "info" => Duplicate::Info,
        "warn" => Duplicate::Warn,
        "error" => Duplicate::Error,
        _ => Duplicate::All,
    }
}

use log::{Log, Metadata};

struct ModuleLevel {
    name: String,
    level: Level,
}

// 过滤掉一些基础模块的trace日志
struct FilterLog {
    logger: Box<dyn Log>,
    mod_levels: Vec<ModuleLevel>,
}

impl FilterLog {
    fn new(logger: Box<dyn Log>) -> Self {
        let mut ret = Self {
            logger,
            mod_levels: Vec::new(),
        };
        ret.disable_async_std_log();

        ret
    }
    // 屏蔽一些基础库的trace log等
    fn disable_async_std_log(&mut self) {
        self.mod_levels.push(ModuleLevel {
            name: "async_io".to_owned(),
            level: Level::Info,
        });
        self.mod_levels.push(ModuleLevel {
            name: "polling".to_owned(),
            level: Level::Info,
        });
        self.mod_levels.push(ModuleLevel {
            name: "async_tungstenite".to_owned(),
            level: Level::Info,
        });
        self.mod_levels.push(ModuleLevel {
            name: "tungstenite".to_owned(),
            level: Level::Info,
        });
        self.mod_levels.push(ModuleLevel {
            name: "async_std".to_owned(),
            level: Level::Info,
        });
        self.mod_levels.push(ModuleLevel {
            name: "tide".to_owned(),
            level: Level::Info,
        });
    }
}

impl Log for FilterLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.logger.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        let target = record.metadata().target();
        //println!("log target={}", target);

        for item in &self.mod_levels {
            if target.starts_with(&item.name) {
                if record.level() > item.level {
                    return;
                }
            }
        }
        self.logger.log(record);
    }

    fn flush(&self) {
        self.logger.flush();
    }
}

pub struct ModuleLog {
    log_dir: PathBuf,

    main_logger: Box<dyn Log>,
    bdt_logger: Option<Box<dyn Log>>,
}

impl ModuleLog {
    pub fn new(
        log_dir: &Path,
        log_level: Option<&str>,
        screen_level: Option<&str>,
    ) -> BuckyResult<Self> {
        let main_logger = Self::new_logger("main", log_dir, log_level, screen_level)?;

        let ret = Self {
            log_dir: log_dir.to_owned(),
            main_logger,
            bdt_logger: None,
        };

        Ok(ret)
    }

    pub fn enable_bdt(&mut self, log_level: Option<&str>, screen_level: Option<&str>) {
        assert!(self.bdt_logger.is_none());

        if let Ok(logger) = Self::new_logger("bdt", &self.log_dir, log_level, screen_level) {
            self.bdt_logger = Some(logger);
        }
    }

    pub fn start(self) {
        // 捕获所有的panic
        ::log_panics::init();

        let logger = Box::new(self) as Box<dyn Log>;
        if let Err(e) = log::set_boxed_logger(logger) {
            let msg = format!("call set_boxed_logger failed! {}", e);
            println!("{}", msg);
        }
    }

    // 默认日志级别
    fn default_log_level(log_level: Option<&str>) -> &str {
        #[cfg(debug_assertions)]
        let log_default_level = "debug";

        #[cfg(not(debug_assertions))]
        let log_default_level = "info";

        match log_level {
            Some(level) if level.len() > 0 => level,
            _ => log_default_level,
        }
    }

    /*
    // 目前优先使用flexi_logger的RUST_LOG环境变量来统一控制
    // 日志级别优先级：ENV > 参数 > 默认值
    // 子模块会默认使用main模块日志级别，除非有独立的配置
    fn env_log_level(log_level: Option<&str>, mod_name: &str) -> String {
        let default_log_level = Self::default_log_level(log_level);

        // 首先获取主模块的env配置
        let env_log_level = std::env::var("CYFS_LOG_LEVEL").unwrap_or_else(|_| default_log_level.to_owned());

        if mod_name != "main" {
            let var_name = format!("CYFS_{}_LOG_LEVEL", mod_name.to_uppercase());
            std::env::var(&var_name).unwrap_or_else(|_| env_log_level)
        }else {
            env_log_level
        }
    }
    */

    fn new_logger(
        mod_name: &str,
        log_dir: &Path,
        log_level: Option<&str>,
        screen_level: Option<&str>,
    ) -> BuckyResult<Box<dyn Log>> {
        let log_level = Self::default_log_level(log_level);

        let discriminant = if mod_name == "main" {
            std::process::id().to_string()
        } else {
            format!("{}_{}", mod_name, std::process::id())
        };

        let file_spec = flexi_logger::FileSpec::default()
            .directory(log_dir)
            .discriminant(discriminant)
            .suppress_timestamp();

        let mut logger = Logger::try_with_env_or_str(log_level)
            .map_err(|e| {
                let msg = format!(
                    "init logger from env or str error! level={:?}, {}",
                    log_level, e
                );
                println!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .log_to_file(file_spec)
            .rotate(
                Criterion::Size(1024 * 1024 * 10),
                Naming::Numbers,
                Cleanup::KeepLogFiles(20),
            )
            .format_for_files(opt_format)
            .duplicate_to_stderr(str_to_duplevel(screen_level.unwrap_or("all")));

        // #[cfg(windows)]
        // {
        //use flexi_logger::detailed_format;
        logger = logger.format_for_stderr(cyfs_default_format);
        //}
        // #[cfg(not(windows))]
        // {
        //     use flexi_logger::colored_detailed_format;
        //     logger = logger.format_for_stderr(colored_detailed_format);
        // }
        #[cfg(feature = "colors")]
        {
            logger = logger.format_for_stderr(cyfs_colored_default_format);
        }
        let (logger, _handle) = logger.build().map_err(|e| {
            let msg = format!("init logger failed! {}", e);
            println!("{}", msg);

            BuckyError::from(msg)
        })?;

        let logger = FilterLog::new(logger);
        Ok(Box::new(logger))
    }
}

impl Log for ModuleLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.main_logger.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        //println!("{:?}", record);
        if let Some(bdt_logger) = &self.bdt_logger {
            let target = record.metadata().target();
            if target.starts_with("cyfs_bdt::") {
                bdt_logger.log(record);
                return;
            }
        }

        self.main_logger.log(record);
    }

    fn flush(&self) {
        self.main_logger.flush();
        if let Some(bdt_logger) = &self.bdt_logger {
            bdt_logger.flush();
        }
    }
}

fn init_log_internal(log_dir: &Path, log_level: Option<&str>) {
    // 捕获所有的panic
    ::log_panics::init();

    if let Ok(logger) = ModuleLog::new_logger("main", log_dir, log_level, None) {
        if let Err(e) = log::set_boxed_logger(logger) {
            println!("init default file log failed! {}", e);
        }
    }
}

/*
#[deprecated(
    note = "Please use the cyfs_debug::CyfsLogger instead"
)]
*/
pub fn init_log(service_name: &str, log_level: Option<&str>) {
    init_log_internal(&get_log_dir(service_name), log_level);
}

pub fn create_log_with_isolate_bdt(
    service_name: &str,
    log_level: Option<&str>,
    bdt_log_level: Option<&str>,
) -> BuckyResult<ModuleLog> {
    let mut mod_log = ModuleLog::new(&get_log_dir(service_name), log_level.clone(), None)?;
    mod_log.enable_bdt(bdt_log_level, None);
    Ok(mod_log)
}

#[deprecated(
    note = "Please use the cyfs_debug::CyfsLogger instead"
)]
pub fn init_log_with_isolate_bdt(
    service_name: &str,
    log_level: Option<&str>,
    bdt_log_level: Option<&str>,
) {
    if let Ok(mod_log) = create_log_with_isolate_bdt(service_name, log_level, bdt_log_level) {
        mod_log.start();
    }
}

#[deprecated(
    note = "Please use the cyfs_debug::CyfsLogger instead"
)]
pub fn init_log_with_isolate_bdt_screen(
    service_name: &str,
    log_level: Option<&str>,
    bdt_log_level: Option<&str>,
    screen_level: Option<&str>,
) {
    if let Ok(mut mod_log) =
        ModuleLog::new(&get_log_dir(service_name), log_level.clone(), screen_level)
    {
        mod_log.enable_bdt(bdt_log_level, screen_level);
        mod_log.start();
    }
}

#[deprecated(
    note = "Please use the cyfs_debug::CyfsLogger instead"
)]
pub fn init_log_with_path(log_dir: &Path, log_level: Option<&str>) {
    init_log_internal(log_dir, log_level);
}

#[deprecated(
    note = "Please use the cyfs_debug::CyfsLogger instead"
)]
pub fn init_app_log(app_name: &str, log_level: Option<&str>) {
    init_log_internal(&get_app_log_dir(app_name), log_level);
}


pub fn flush_log() {
    log::logger().flush();
}