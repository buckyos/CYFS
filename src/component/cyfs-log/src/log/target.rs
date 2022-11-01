use super::constants::*;
use cyfs_base::*;

use log::Record;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CyfsLogRecord {
    level: LogLevel,
    target: String,
    time: u64,
    file: Option<String>,
    line: Option<u32>,
    content: String,
}

impl CyfsLogRecord {
    pub fn new(record: &Record) -> Self {
        let level: LogLevel = record.metadata().level().into();
        let target = record.metadata().target().to_owned();
        let time = cyfs_base::bucky_time_now();

        let content = format!("{}", record.args());

        Self {
            level,
            target,
            time,
            file: record.file().map(|v| v.to_owned()),
            line: record.line(),
            content,
        }
    }

    pub fn easy_log(level: LogLevel, content: String) -> Self {
        Self {
            level,
            time: cyfs_base::bucky_time_now(),
            target: "".to_string(),
            file: None,
            line: None,
            content,
        }
    }

    pub fn content(&self) -> String {
        self.content.clone()
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn time(&self) -> u64 {
        self.time
    }

    pub fn file(&self) -> String {
        match &self.file {
            Some(f) => f.clone(),
            _ => "".to_string(),
        }
    }

    pub fn line(&self) -> u32 {
        match &self.line {
            Some(l) => *l,
            _ => 0,
        }
    }
}

use chrono::offset::Local;
use chrono::DateTime;


impl std::fmt::Display for CyfsLogRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let system_time = bucky_time_to_system_time(self.time);
        let datetime: DateTime<Local> = system_time.into();
        let time_str = datetime.format("%Y-%m-%d %H:%M:%S%.3f %:z");

        write!(
            f,
            "[{}] {} [{}:{}] {}",
            time_str,
            self.level.to_string().to_uppercase(),
            self.file.as_deref().unwrap_or("<unnamed>"),
            self.line.unwrap_or(0),
            self.content,
        )
    }
}

pub trait CyfsLogTarget: Send + Sync {
    fn log(&self, record: &CyfsLogRecord);
}

pub struct ConsoleCyfsLogTarget {}

impl CyfsLogTarget for ConsoleCyfsLogTarget {
    fn log(&self, record: &CyfsLogRecord) {
        println!(">>>{}", record);
    }
}
