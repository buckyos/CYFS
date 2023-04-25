use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileProgress {
    pub file: String,
    pub total: u64,
    pub completed: u64,
    pub result: Option<BuckyResult<()>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArchiveProgress {
    pub total: u64,
    pub completed: u64,
    pub result: Option<BuckyResult<()>>,

    pub current: Option<FileProgress>,
}

impl ArchiveProgress {
    pub fn new() -> Self {
        Self {
            total: 0,
            completed: 0,
            result: None,
            current: None,
        }
    }

    pub fn reset_total(&mut self, total: u64) {
        self.total = total;    
    }

    pub fn finish(&mut self, result: BuckyResult<()>) {
        assert!(self.result.is_none());
        self.result = Some(result);
    }

    pub fn begin_file(&mut self, file: &str, total: u64) {
        // Treat as single file archive mode
        if self.total == 0 {
            self.total = total;
        }

        let file_progress = FileProgress {
            file: file.to_owned(),
            total,
            completed: 0,
            result: None,
        };

        self.current = Some(file_progress);
    }

    pub fn reset_current_file_total(&mut self, total: u64) {
        // Treat as single file archive mode
        if self.total == 0 {
            self.total = total;
        }

        assert!(self.current.is_some());
        self.current.as_mut().unwrap().total = total;
    }

    pub fn update_current_file_progress(&mut self, completed: u64) {
        assert!(self.current.is_some());
        self.current.as_mut().unwrap().completed = completed;
        self.completed += completed;
    }

    pub fn finish_current_file(&mut self, result: BuckyResult<()>) {
        assert!(self.current.is_some());
        self.current.as_mut().unwrap().result = Some(result);
        self.current = None;
    }
}


#[derive(Clone)]
pub struct ArchiveProgessHolder(Arc<Mutex<ArchiveProgress>>);

impl ArchiveProgessHolder {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(ArchiveProgress::new())))
    }

    pub fn reset_total(&self, total: u64) {
        self.0.lock().unwrap().reset_total(total)
    }

    pub fn finish(&self, result: BuckyResult<()>) {
        self.0.lock().unwrap().finish(result)
    }

    // File progress related methods
    pub fn begin_file(&self, file: &str, total: u64) {
        self.0.lock().unwrap().begin_file(file, total)
    }

    pub fn reset_current_file_total(&self, total: u64) {
        self.0.lock().unwrap().reset_current_file_total(total)
    }

    pub fn update_current_file_progress(&self, completed: u64) {
        self.0.lock().unwrap().update_current_file_progress(completed)
    }

    pub fn finish_current_file(&self, result: BuckyResult<()>) {
        self.0.lock().unwrap().finish_current_file(result)
    }
}
