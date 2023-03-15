use super::backup_status::BackupStatInfo;
use crate::{archive::*, meta::*};
use cyfs_base::*;

use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub enum RestoreTaskPhase {
    Init,
    LoadAndVerify,
    RestoreKeyData,
    RestoreObject,
    RestoreChunk,
    Complete,
}

impl Default for RestoreTaskPhase {
    fn default() -> Self {
        Self::Init
    }
}

pub type RestoreStatInfo = BackupStatInfo;

#[derive(Clone, Debug)]
pub struct RestoreResult {
    pub index: ObjectArchiveIndex,
    pub uni_meta: Option<ObjectArchiveMetaForUniBackup>,
}

#[derive(Clone, Debug, Default)]
pub struct RestoreStatus {
    pub phase: RestoreTaskPhase,
    pub phase_last_update_time: u64,

    pub stat: RestoreStatInfo,
    pub complete: RestoreStatInfo,

    pub result: Option<BuckyResult<RestoreResult>>,
}

#[derive(Clone)]
pub struct RestoreStatusManager {
    status: Arc<Mutex<RestoreStatus>>,
}

impl RestoreStatusManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(Mutex::new(RestoreStatus::default())),
        }
    }

    pub fn status(&self) -> RestoreStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn init_stat(&self, meta: &ObjectArchiveMetaForUniBackup) {
        let mut files = ObjectArchiveDataMeta::default();
        for item in &meta.key_data {
            files.count += 1;
            files.bytes += item.chunk_id.len() as u64;
        }

        let objects = meta.object.meta.data.objects.clone();
        let chunks = meta.object.meta.data.chunks.clone();

        let stat = RestoreStatInfo {
            files,
            objects,
            chunks,
        };

        let mut status = self.status.lock().unwrap();
        status.stat = stat;
    }

    pub fn update_phase(&self, phase: RestoreTaskPhase) -> RestoreTaskPhase {
        let mut status = self.status.lock().unwrap();
        let cur = status.phase;
        status.phase = phase;
        status.phase_last_update_time = bucky_time_now();

        cur
    }

    pub fn on_file(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.files.count += 1;
    }

    pub fn on_object(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.objects.count += 1;
    }

    pub fn on_chunk(&self) {
        let mut status = self.status.lock().unwrap();
        status.complete.chunks.count += 1;
    }

    pub fn on_complete(&self, ret: BuckyResult<RestoreResult>) {
        let mut status = self.status.lock().unwrap();
        status.result = Some(ret);
    }
}
