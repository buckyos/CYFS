use cyfs_base::*;

use file_rotate::{compression::Compression, suffix::AppendCount, ContentLimit, FileRotate};
use std::sync::Mutex;
use std::{io::Write, path::PathBuf};

struct BackupLogFile {
    writer: FileRotate<AppendCount>,
}

impl BackupLogFile {
    pub fn new(file: PathBuf) -> Self {
        let writer = FileRotate::new(
            file,
            AppendCount::new(1024),
            ContentLimit::BytesSurpassed(1024 * 1024 * 10),
            Compression::None,
        );

        Self { writer }
    }

    pub fn output_line(&mut self, line: &str) -> BuckyResult<()> {
        self.writer.write_all(line.as_bytes()).map_err(|e| {
            let msg = format!("write backup log failed! msg={}, {}", line, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })
    }
}

pub struct BackupLogManager {
    default_isolate: Option<ObjectId>,
    error: Mutex<BackupLogFile>,
    missing: Mutex<BackupLogFile>,
}

impl BackupLogManager {
    pub fn new(default_isolate: Option<ObjectId>, dir: PathBuf) -> Self {
        let file = dir.join("error.log");
        let error = BackupLogFile::new(file);

        let file = dir.join("missing.log");
        let missing = BackupLogFile::new(file);

        Self {
            default_isolate,
            error: Mutex::new(error),
            missing: Mutex::new(missing),
        }
    }

    pub fn on_error(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        id: &ObjectId,
        e: BuckyError,
    ) {
        let msg = match isolate_id {
            Some(isolate_id) => {
                let dec_id = dec_id.unwrap();
                if self.default_isolate == Some(*isolate_id) {
                    format!("[{}] [{}] {}", dec_id, id, e)
                } else {
                    format!("[{}] [{}] [{}] {}", isolate_id, dec_id, id, e)
                }
            }
            None => {
                format!("[{}] {}", id, e)
            }
        };

        let _ = self.error.lock().unwrap().output_line(&msg);
    }

    pub fn on_missing(
        &self,
        isolate_id: Option<&ObjectId>,
        dec_id: Option<&ObjectId>,
        id: &ObjectId,
    ) {
        let msg = match isolate_id {
            Some(isolate_id) => {
                let dec_id = dec_id.unwrap();
                if self.default_isolate == Some(*isolate_id) {
                    format!("[{}] [{}]", dec_id, id,)
                } else {
                    format!("[{}] [{}] [{}]", isolate_id, dec_id, id)
                }
            }
            None => {
                format!("[{}]", id)
            }
        };

        let _ = self.missing.lock().unwrap().output_line(&msg);
    }
}
