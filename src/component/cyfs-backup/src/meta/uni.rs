use cyfs_backup_lib::*;
use cyfs_base::*;


use std::sync::{Arc, Mutex};
use std::ops::DerefMut;


#[derive(Clone)]
pub struct ObjectArchiveUniMetaHolder {
    meta: Arc<Mutex<ObjectArchiveUniMeta>>,
}

impl ObjectArchiveUniMetaHolder {
    pub fn new() -> Self {
        let meta = ObjectArchiveUniMeta::new();

        Self {
            meta: Arc::new(Mutex::new(meta)),
        }
    }

    pub fn on_error(&self, id: &ObjectId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_error(id)
    }

    pub fn on_missing(&self, id: &ObjectId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_missing(id)
    }

    pub fn on_object(&self, bytes: usize) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_object(bytes)
    }

    pub fn on_chunk(&self, chunk_id: &ChunkId) {
        let mut meta = self.meta.lock().unwrap();
        meta.meta.on_chunk(chunk_id)
    }

    pub fn finish(&self) -> ObjectArchiveUniMeta {
        let meta = {
            let mut meta = self.meta.lock().unwrap();
            let mut empty_meta = ObjectArchiveUniMeta::new();
            std::mem::swap(meta.deref_mut(), &mut empty_meta);

            empty_meta
        };

        meta
    }
}
