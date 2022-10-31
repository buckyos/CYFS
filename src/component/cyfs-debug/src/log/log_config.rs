use super::LogLevel;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_util::TomlHelper;

use std::{collections::{HashMap, hash_map::Entry}, str::FromStr};
use std::path::{Path, PathBuf};

// 日志环境变量
const CYFS_CONSOLE_LOG_LEVEL_KEY: &str = "CYFS_CONSOLE_LOG_LEVEL";
const CYFS_FILE_LOG_LEVEL_KEY: &str = "CYFS_FILE_LOG_LEVEL";

#[derive(Clone)]
pub struct LogModuleConfig {
    pub name: String,

    pub level: LogLevel,

    // 是否输出控制台日志
    pub console: LogLevel,

    // 是否输出文件日志
    pub file: bool,

    // 是否使用独立文件
    pub file_name: Option<String>,

    // 单个日志文件的最大大小，字节
    pub file_max_size: u64,

    // 日志文件最大个数，滚动输出
    pub file_max_count: u32,
}

impl LogModuleConfig {
    pub fn new_default(name: &str) -> Self {
        Self {
            name: name.to_owned(),

            level: LogLevel::default(),
            console: LogLevel::default(),
            file: true,
            file_name: Some(name.to_string()),
            file_max_size: 1024 * 1024 * 10,
            file_max_count: 10,
        }
    }

    pub fn is_global_module(&self) -> bool {
        self.name == "global"
    }

    pub fn max_level(&self) -> LogLevel {
        std::cmp::max(&self.level, &self.console).to_owned()
    }
    
    pub fn set_level(&mut self, level: &str) {
        self.level = LogLevel::from_str(level).expect(&format!("invalid level str: {}", level));
    }

    pub fn set_console(&mut self, level: &str) {
        self.console = LogLevel::from_str(level).expect(&format!("invalid level str: {}", level));
    }

    pub fn set_file(&mut self, enable: bool) {
        self.file = enable;
    }

    pub fn set_file_max_size(&mut self, file_max_size: u64) {
        self.file_max_size = file_max_size;
    }

    pub fn set_file_max_count(&mut self, file_max_count: u32) {
        self.file_max_count = file_max_count;
    }

    // 加载环境变量配置的日志级别
    fn load_console_level_from_env() -> Option<LogLevel> {
        match std::env::var(CYFS_CONSOLE_LOG_LEVEL_KEY) {
            Ok(val) => {
                match LogLevel::from_str(&val) {
                    Ok(level) => Some(level),
                    Err(e) => {
                        println!("parse env log level error! {}, {}", val, e);
                        None
                    }
                }
            }
            Err(_) => {
                None
            }
        }
    }

    fn load_file_level_from_env() -> Option<LogLevel> {
        match std::env::var(CYFS_FILE_LOG_LEVEL_KEY) {
            Ok(val) => {
                match LogLevel::from_str(&val) {
                    Ok(level) => Some(level),
                    Err(e) => {
                        println!("parse env log level error! {}, {}", val, e);
                        None
                    }
                }
            }
            Err(_) => {
                None
            }
        }
    }

    pub fn load(&mut self, node: &toml::value::Table) {
        let mut console: Option<LogLevel> = None;
        let mut level: Option<LogLevel> = None;
        for (k, v) in node {
            match k.as_str() {
                "console" => {
                    if let Ok(v) = TomlHelper::decode_from_string(v) {
                        console = Some(v);
                    }
                }

                "file" => {
                    if let Ok(v) = TomlHelper::decode_from_string::<String>(v) {
                        if v.as_str() == "off" {
                            self.file = false;
                            self.file_name = None;
                        } else {
                            self.file = true;
                            self.file_name = Some(v);
                        }
                    }
                }

                "level" => {
                    if let Ok(v) = TomlHelper::decode_from_string(v) {
                        level = Some(v);
                    }
                }

                "file_max_size" => {
                    match TomlHelper::decode_to_int::<u64>(v) {
                        Ok(v) => self.file_max_size = v,
                        Err(e) => println!("decode log.toml file_max_size field error! {}", e),
                    }
                }

                "file_max_count" => {
                    match TomlHelper::decode_to_int::<u32>(v) {
                        Ok(v) => self.file_max_count = v,
                        Err(e) => println!("decode log.toml file_max_count field error! {}", e),
                    }
                }

                v @ _ => {
                    println!("unknown module config field: {}", v);
                }
            }
        }

        // 如果只配置了level，但没配置console，那么console默认使用配置的level，而不是代码内置的debug级别
        if console.is_none() && level.is_some() {
            console = level.clone();
        }

        if let Some(v) = level {
            self.level = v;
        }
        if let Some(v) = console {
            self.console = v;
        }

        // 尝试读取环境变量来配置
        if let Some(level) = Self::load_console_level_from_env() {
            println!("load module console level from env: mod={}, level={}, old={}", self.name, level, self.console);
            self.console = level;
        }

        if let Some(level) = Self::load_file_level_from_env() {
            println!("load module file level from env: mod={}, level={}, old={}", self.name, level, self.level);
            self.level = level;
        }
    }
}

pub struct LogConfig {
    pub log_dir: PathBuf,
    pub basename: String,

    pub global: LogModuleConfig,
    pub modules: HashMap<String, LogModuleConfig>,
}

impl LogConfig {
    pub fn new(log_dir: PathBuf) -> Self {
        let arg0 = std::env::args().next().unwrap_or_else(|| "main".to_owned());
        let basename = Path::new(&arg0).file_stem().unwrap(/*cannot fail*/).to_string_lossy().to_string();

        Self {
            log_dir,
            global: LogModuleConfig::new_default("global"),
            modules: HashMap::new(),
            basename,
        }
    }

    pub fn set_log_dir(&mut self, log_dir: PathBuf) {
        self.log_dir = log_dir;
    }
    
    pub fn get_mod_config(&self, name: Option<&str>) -> &LogModuleConfig {
        match name {
            None => {
                &self.global
            }
            Some(name) => {
                if let Some(m) = self.modules.get(name) {
                    return m;
                }
        
                &self.global
            }
        }
    }

    pub fn load(&mut self, config_node: &toml::Value) -> BuckyResult<()> {
        
        let node = config_node.as_table().ok_or_else(|| {
            let msg = format!(
                "invalid log config format! content={}",
                config_node,
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        for (k, v) in node {
            if !v.is_table() {
                error!("invalid config node format: {}", v);
                continue;
            }

            let v = v.as_table().unwrap();
            match k.as_str() {
                "global" => {
                    self.global.load(v);
                }

                key @ _ => {
                    self.load_mod(key, v);
                }
            }
        }

        Ok(())
    }

    fn load_mod(&mut self, name: &str, node: &toml::value::Table) {
        let mut module = LogModuleConfig::new_default(name);
        module.load(node);

       self.add_mod(module);
    }

    pub fn add_mod(&mut self, module: LogModuleConfig) {
        if let Some(old) = self.modules.insert(module.name.clone(), module) {
            println!("replace old log module: {}={}", old.name, old.level);
        }
    }

    pub fn disable_module_log(&mut self, name: &str, level: &LogLevel) {
        let mut module = LogModuleConfig::new_default(name);
        module.level = *level;
        module.console = *level;
        module.file_name = None;

        // 如果已经提前配置了，那么不需要覆盖
        match self.modules.entry(name.to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert(module);
            },
            Entry::Occupied(mut _entry)=>{
                println!("module already in config: name={}", name);
            }
        };
    }

    // 屏蔽一些基础库的trace log等
    pub fn disable_async_std_log(&mut self) {
        let mod_list = [
            ("async_io", LogLevel::Warn),
            ("polling", LogLevel::Warn),
            ("async_tungstenite", LogLevel::Warn),
            ("tungstenite", LogLevel::Warn),
            ("async_std", LogLevel::Warn),
            ("tide", LogLevel::Warn),
        ];

        mod_list.iter().for_each(|(name, level)| {
            self.disable_module_log(name, level);
        })
    }
}
