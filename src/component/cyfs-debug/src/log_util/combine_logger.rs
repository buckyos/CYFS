use log::*;

pub fn str_to_logfilter_level(level: &str) -> LevelFilter {
    match level {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        "off" => LevelFilter::Off,
        _ => LevelFilter::Off,
    }
}

pub fn str_to_log_level(level: &str) -> Level {
    match level {
        "trace" => Level::Trace,
        "debug" => Level::Debug,
        "info" => Level::Info,
        "warn" => Level::Warn,
        "error" => Level::Error,
        _ => Level::Error,
    }
}

pub struct CombineLogger {
    loggers: Vec<Box<dyn Log>>
}

impl CombineLogger {
    pub fn new() -> CombineLogger
    {
        CombineLogger{loggers: vec![]}
    }

    pub fn append(mut self, logger: Box<dyn Log>) -> Self {
        self.loggers.push(logger);
        self
    }
    
    pub fn start(self) -> bool {
        // 处理多个模块都嵌有Logger的情况，如果已经设置过logger，这个函数不起作用，返回false，以第一次创建的为准
        // 成功注册，则返回true
        if let Err(e) = log::set_boxed_logger(Box::new(self)) {
            warn!("logger already initialized, use prev one. {}", e);
            false
        } else {
            true
        }
    }
}

impl Log for CombineLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let mut ret = true;
        for logger in &self.loggers {
            ret = ret | logger.enabled(metadata);
        }
        ret
    }

    fn log(&self, record: &Record) {
        for logger in &self.loggers {
            logger.log(record);
        }
    }

    fn flush(&self) {
        for logger in &self.loggers {
            logger.flush();
        }
    }
}