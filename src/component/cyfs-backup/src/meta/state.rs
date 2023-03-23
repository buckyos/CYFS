use cyfs_backup_lib::*;

use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ObjectArchiveStateMetaHolder {
    meta: Arc<Mutex<ObjectArchiveStateMeta>>,
}

impl ObjectArchiveStateMetaHolder {
    pub fn new() -> Self {
        let meta = ObjectArchiveStateMeta::new();

        Self {
            meta: Arc::new(Mutex::new(meta)),
        }
    }

    pub fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        let mut archive = self.meta.lock().unwrap();
        archive.add_isolate(isolate_meta);
    }

    pub fn finish(&self) -> ObjectArchiveStateMeta {
        let meta = {
            let mut meta = self.meta.lock().unwrap();
            let mut empty_meta = ObjectArchiveStateMeta::new();
            std::mem::swap(meta.deref_mut(), &mut empty_meta);

            empty_meta
        };

        meta
    }
}
