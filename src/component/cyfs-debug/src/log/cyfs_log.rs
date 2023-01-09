use super::constants::*;
use super::flexi_log::*;
use super::log_config::*;
use super::target::*;
use crate::debug_config::*;
use cyfs_base::BuckyResult;
use cyfs_util::get_cyfs_root_path;

use log::Log;
use std::path::PathBuf;
use std::str::FromStr;

pub struct CyfsLogger {
    config: LogConfig,
    logger: FlexiLogger,
}

pub enum CyfsLoggerCategory {
    Service,
    App,
}

pub struct CyfsLoggerBuilder {
    config: LogConfig,
    targets: Vec<Box<dyn CyfsLogTarget>>,
    disable_file_config: bool,
}

impl CyfsLoggerBuilder {
    pub fn new_service(name: &str) -> Self {
        Self::new(name, CyfsLoggerCategory::Service)
    }

    pub fn new_app(name: &str) -> Self {
        Self::new(name, CyfsLoggerCategory::App)
    }

    pub fn new(name: &str, category: CyfsLoggerCategory) -> Self {
        // simple_logger::SimpleLogger::default().init().unwrap();

        let log_dir = Self::get_log_dir(name, &category);
        let config = LogConfig::new(log_dir);
        Self {
            config,
            targets: vec![],
            disable_file_config: false,
        }
    }

    pub fn directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config.set_log_dir(dir.into());
        self
    }

    pub fn level(mut self, level: &str) -> Self {
        self.config.global.set_level(level);
        self
    }

    pub fn console(mut self, level: &str) -> Self {
        self.config.global.set_console(level);
        self
    }

    pub fn file(mut self, enable: bool) -> Self {
        self.config.global.file = enable;
        self
    }

    pub fn file_max_count(mut self, file_max_count: u32) -> Self {
        self.config.global.set_file_max_count(file_max_count);
        self
    }

    pub fn file_max_size(mut self, file_max_size: u64) -> Self {
        self.config.global.set_file_max_size(file_max_size);
        self
    }

    pub fn enable_bdt(mut self, level: Option<&str>, console_level: Option<&str>) -> Self {
        let config =
            Self::bdt_module(level, console_level).expect(&format!("invalid bdt log config"));
        self.config.add_mod(config);

        self
    }

    pub fn module(mut self, name: &str, level: Option<&str>, console_level: Option<&str>) -> Self {
        let config = Self::new_module(name, name, level, console_level)
            .expect(&format!("invalid module log config"));
        self.config.add_mod(config);

        self
    }

    pub fn target(mut self, target: Box<dyn CyfsLogTarget>) -> Self {
        self.targets.push(target);
        self
    }

    pub fn disable_module(mut self, list: Vec<impl Into<String>>, level: LogLevel) -> Self {
        for name in list {
            let name = name.into();
            self.config.disable_module_log(&name, &level);
        }
        self
    }

    // do not use {cyfs}/etc/debug.toml
    pub fn disable_file_config(mut self, disable: bool) -> Self {
        self.disable_file_config = disable;
        self
    }

    pub fn build(mut self) -> BuckyResult<CyfsLogger> {
        self.config.disable_async_std_log();

        if !self.disable_file_config {
            if let Some(config_node) = DebugConfig::get_config("log") {
                if let Err(e) = self.config.load(config_node) {
                    println!("load log config error! {}", e);
                }
            }
        }

        let logger = FlexiLogger::new(&self.config, self.targets)?;

        let ret = CyfsLogger {
            config: self.config,
            logger,
        };

        Ok(ret)
    }

    pub fn get_log_dir(name: &str, category: &CyfsLoggerCategory) -> PathBuf {
        assert!(!name.is_empty());

        let mut root = get_cyfs_root_path();
        let folder = match *category {
            CyfsLoggerCategory::Service => "log",
            CyfsLoggerCategory::App => "log/app",
        };
        root.push(folder);

        root.push(name);
        root
    }

    fn new_module(
        name: &str,
        file_name: &str,
        level: Option<&str>,
        console_level: Option<&str>,
    ) -> BuckyResult<LogModuleConfig> {
        let mut config = LogModuleConfig::new_default(name);
        if let Some(level) = level {
            config.level = LogLevel::from_str(level)?;
        }
        if let Some(level) = console_level {
            config.console = LogLevel::from_str(level)?;
        }

        config.file_name = Some(file_name.to_owned());
        Ok(config)
    }

    fn bdt_module(
        level: Option<&str>,
        console_level: Option<&str>,
    ) -> BuckyResult<LogModuleConfig> {
        Self::new_module("cyfs_bdt", "bdt", level, console_level)
    }
}

impl Into<Box<dyn Log>> for CyfsLogger {
    fn into(self) -> Box<dyn Log> {
        Box::new(self.logger) as Box<dyn Log>
    }
}

impl CyfsLogger {
    pub fn start(self) {
        let max_level = self.logger.get_max_level();
        println!("log max level: {}", max_level);
        log::set_max_level(max_level.into());

        if let Err(e) = log::set_boxed_logger(self.into()) {
            let msg = format!("call set_boxed_logger failed! {}", e);
            println!("{}", msg);
        }

        Self::display_debug_info();
    }

    pub fn display_debug_info() {
        // 输出环境信息，用以诊断一些环境问题
        for argument in std::env::args() {
            info!("arg: {}", argument);
        }

        // info!("current exe: {:?}", std::env::current_exe());
        info!("current dir: {:?}", std::env::current_dir());

        info!("current version: {}", cyfs_base::get_version());

        for (key, value) in std::env::vars() {
            info!("env: {}: {}", key, value);
        }
    }

    pub fn flush() {
        log::logger().flush();
    }
}
