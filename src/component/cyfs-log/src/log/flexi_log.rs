use super::constants::*;
use super::log_config::*;
use super::target::*;
use cyfs_base::{BuckyError, BuckyResult};

use flexi_logger::{
    opt_format, Cleanup, Criterion, DeferredNow, LogSpecification, Logger, Naming, Record,
};
use log::{Log, Metadata};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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

struct FlexiModuleLogger {
    config: LogModuleConfig,

    logger: Arc<Box<dyn Log>>,
}

impl FlexiModuleLogger {
    pub fn new(log_dir: &Path, config: &LogModuleConfig) -> BuckyResult<Self> {
        let mut config = config.clone();
        let logger = Self::new_logger(log_dir, &mut config)?;

        Ok(Self {
            config,
            logger: Arc::new(logger),
        })
    }

    pub fn clone_with_config(&self, config: &LogModuleConfig) -> Self {
        Self {
            config: config.clone(),
            logger: self.logger.clone(),
        }
    }

    fn check_level(&self, level: log::Level) -> bool {
        level as usize <= self.config.level as usize
    }

    fn new_spec(config: &mut LogModuleConfig) -> LogSpecification {
        if let Ok(spec) = std::env::var("RUST_LOG") {
            if let Ok(ret) = LogSpecification::parse(&spec) {
                // 从环境变量读取log_level后，需要反向更新配置
                let mut level = None;
                for m in ret.module_filters() {
                    // 判断mod和当前mod是否匹配
                    match &m.module_name {
                        Some(name) => {
                            if *name == config.name {
                                level = Some(m.level_filter);
                                break;
                            }
                        }
                        None => {
                            if config.is_global_module() {
                                level = Some(m.level_filter);
                                break;
                            }
                        }
                    }
                }

                // 区分模块
                if let Some(level) = level {
                    config.level = level.into();
                    config.console = config.level.clone();

                    println!(
                        "use RUST_LOG env for module: {} = {}",
                        config.name, config.level
                    );

                    return ret;
                }

                /*
                let max_level = ret
                    .module_filters()
                    .iter()
                    .map(|d| {
                        println!("RUST_LOG env: {:?} : {:?}", d.module_name, d.level_filter);
                        d.level_filter
                    })
                    .max()
                    .unwrap_or(log::LevelFilter::Off);
                config.level = max_level.into();
                config.console = config.level.clone();

                return ret;
                */
            } else {
                println!(
                    "parse RUST_LOG env failed! module={}, spec={}",
                    config.name, spec
                );
            }
        }

        println!(
            "new logger: mod={}, level={}",
            config.name,
            config.max_level()
        );

        flexi_logger::LogSpecBuilder::from_module_filters(&[flexi_logger::ModuleFilter {
            module_name: None,
            level_filter: config.max_level().into(),
        }])
        .build()

        // LogSpecification::default(config.level.clone().into()).build()
    }

    fn new_logger(log_dir: &Path, config: &mut LogModuleConfig) -> BuckyResult<Box<dyn Log>> {
        println!(
            "new logger: dir={}, name={}, level={}, console={}",
            log_dir.display(),
            config.name,
            config.level,
            config.console,
        );

        let discriminant = if config.name == "global" {
            std::process::id().to_string()
        } else {
            let file_name = match &config.file_name {
                Some(v) => v.as_str(),
                None => config.name.as_str(),
            };
            format!("{}_{}", file_name, std::process::id())
        };

        let spec = Self::new_spec(config);
        let file_spec = flexi_logger::FileSpec::default()
            .directory(log_dir)
            .discriminant(discriminant)
            .suppress_timestamp();

        let mut logger = Logger::with(spec);

        if config.file {
            logger = logger
                .log_to_file(file_spec)
                .rotate(
                    Criterion::Size(config.file_max_size),
                    Naming::Numbers,
                    Cleanup::KeepLogFiles(config.file_max_count as usize),
                )
                .format_for_files(opt_format);
        }

        if config.console != LogLevel::Off {
            logger = logger.duplicate_to_stderr(config.console.clone().into());
            logger = logger.format_for_stderr(cyfs_default_format);

            #[cfg(feature = "colors")]
            {
                logger = logger.format_for_stderr(cyfs_colored_default_format);
            }
        }

        let (logger, _handle) = logger.build().map_err(|e| {
            let msg = format!("init logger failed! {}", e);
            println!("{}", msg);

            BuckyError::from(msg)
        })?;

        Ok(logger)
    }
}

pub struct FlexiLogger {
    global_logger: FlexiModuleLogger,
    module_loggers: HashMap<String, FlexiModuleLogger>,
    max_level: LogLevel,

    targets: Vec<Box<dyn CyfsLogTarget>>,
}

impl FlexiLogger {
    pub fn new(config: &LogConfig, targets: Vec<Box<dyn CyfsLogTarget>>) -> BuckyResult<Self> {
        let global_logger = FlexiModuleLogger::new(&config.log_dir, &config.global)?;
        let mut max_level = global_logger.config.level;

        let mut module_loggers = HashMap::new();
        for (k, mod_config) in &config.modules {
            // 必须使用logger内部的level
            let level;
            if mod_config.file_name.is_some() {
                if let Ok(logger) = FlexiModuleLogger::new(&config.log_dir, mod_config) {
                    level = logger.config.level;
                    println!("new logger mod with isolate file: {} {}", k, level);
                    module_loggers.insert(k.clone(), logger);
                } else {
                    continue;
                }
            } else {
                let logger = global_logger.clone_with_config(mod_config);
                level = logger.config.level;
                println!("new logger mod clone from global: {} {}", k, level);
                module_loggers.insert(k.clone(), logger);
            }

            if level > max_level {
                max_level = level;
            }
        }

        Ok(Self {
            max_level,
            global_logger,
            module_loggers,
            targets,
        })
    }

    pub fn get_max_level(&self) -> LogLevel {
        self.max_level
    }

    fn get_logger(&self, target: &str) -> &FlexiModuleLogger {
        let mod_name = match target.find("::") {
            Some(pos) => &target[..pos],
            None => target,
        };

        if let Some(item) = self.module_loggers.get(mod_name) {
            item
        } else {
            &self.global_logger
        }
    }
}

impl Log for FlexiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let target = metadata.target();
        let logger = self.get_logger(target);
        logger.check_level(metadata.level())
    }

    fn log(&self, record: &Record) {
        let target = record.metadata().target();
        /*
        if target.starts_with("cyfs_bdt") {
            println!("log target={}, level={}, logger_level={}", target, record.metadata().level(), logger.config.level);
        }
        */
        let logger = self.get_logger(target);
        if logger.check_level(record.metadata().level()) {
            // println!("will output");
            logger.logger.log(record);

            // 如果存在其他的目标，那么输出到目标
            if !self.targets.is_empty() {
                let record = CyfsLogRecord::new(record);
                for target in &self.targets {
                    target.log(&record);
                }
            }
        }
    }

    fn flush(&self) {
        self.global_logger.logger.flush();
        for (_, mod_config) in &self.module_loggers {
            mod_config.logger.flush();
        }
    }
}
