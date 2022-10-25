use log::{Log, Metadata, Record, SetLoggerError, LevelFilter, Level};
use std::fs::File;
use std::io::Write;
use std::sync::Mutex;
use ansi_term::{Color, Style};

pub struct FacadeLogger {
    min_level: log::Level,
    file: Option<Mutex<File>>
}

impl FacadeLogger {
    pub fn new(file_path: Option<&str>) -> Self {
        let file = file_path.map(|path|std::fs::File::create(path).unwrap());
        #[cfg(windows)]
        {
            if let Err(e) = ansi_term::enable_ansi_support() {
                println!("Warning: config Windows Terminal Color Mode Failed. err {}", e);
            }
        }
        Self {
            min_level: log::Level::Trace,
            file: file.map(|f|Mutex::new(f))
        }
    }

    pub fn start(self) -> Result<(), SetLoggerError> {
        log::set_max_level(LevelFilter::Trace);
        log::set_boxed_logger(Box::new(self))
    }
}

impl Log for FacadeLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        // 不输出非本进程的日志
        if !metadata.target().contains("cyfs_stack_bench") {
            return false;
        }
        return metadata.level() <= self.min_level;
    }

    fn log(&self, record: &Record) {
        // 不输出非本进程的日志
        if !record.target().starts_with("cyfs_stack_bench") {
            return;
        }
        // 非Debug级别的log都记录到文件
        if record.level() != Level::Debug {
            if let Some(file) = &self.file {
                file.lock().unwrap().write(format!("{}\n", record.args()).as_bytes()).unwrap();
            }
        }
        // trace级别的日志不输出到控制台
        if record.level() != Level::Trace {
            // 给控制台上颜色
            let color = match record.level() {
                Level::Error => {Style::default().fg(Color::Red).bold()}
                Level::Warn => {Style::default().fg(Color::Yellow).bold()}
                Level::Info => {Style::default().fg(Color::Green)}
                Level::Debug => {Style::default().fg(Color::White)}
                _ => {Style::default()}
            };
            let msg = format!("{}", record.args());

            println!("{}", color.paint(msg));
        }

    }

    fn flush(&self) {
        if let Some(file) = &self.file {
            file.lock().unwrap().flush().unwrap();
        }
    }
}