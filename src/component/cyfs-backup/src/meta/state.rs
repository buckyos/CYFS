use super::data::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDecMeta {
    pub dec_id: ObjectId,
    pub dec_root: ObjectId,

    pub meta: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveDecMeta {
    pub fn new(dec_id: ObjectId, dec_root: ObjectId) -> Self {
        Self {
            dec_id,
            dec_root,

            meta: ObjectArchiveDataSeriesMeta::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveIsolateMeta {
    isolate_id: ObjectId,
    root: ObjectId,
    revision: u64,

    decs: Vec<ObjectArchiveDecMeta>,
}

impl ObjectArchiveIsolateMeta {
    pub fn new(isolate_id: ObjectId, root: ObjectId, revision: u64) -> Self {
        Self {
            isolate_id,
            decs: vec![],
            root,
            revision,
        }
    }

    pub fn add_dec(&mut self, dec_meta: ObjectArchiveDecMeta) {
        assert!(self
            .decs
            .iter()
            .find(|item| item.dec_id == dec_meta.dec_id)
            .is_none());

        self.decs.push(dec_meta);
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveStateMeta {
    pub id: u64,
    pub time: String,

    pub isolates: Vec<ObjectArchiveIsolateMeta>,
    pub roots: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveStateMeta {
    pub fn new(id: u64) -> Self {
        let datetime = chrono::offset::Local::now();
        // let time = datetime.format("%Y-%m-%d %H:%M:%S%.3f %:z");
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            isolates: vec![],
            roots: ObjectArchiveDataSeriesMeta::default(),
        }
    }

    pub fn add_isolate(&mut self, isolate_meta: ObjectArchiveIsolateMeta) {
        self.isolates.push(isolate_meta);
    }

    pub fn add_isolate_dec(&mut self, isolate_id: &ObjectId, dec_meta: ObjectArchiveDecMeta) {
        let ret = self
            .isolates
            .iter_mut()
            .find(|item| item.isolate_id == *isolate_id);
        match ret {
            Some(item) => {
                item.add_dec(dec_meta);
            }
            None => {
                unreachable!();
                // let mut item = ObjectArchiveIsolateMeta::new(isolate_id.to_owned());
                // item.add_dec(dec_id);
                // self.isolates.push(item);
            }
        }
    }
}

#[derive(Clone)]
pub struct ObjectArchiveStateMetaHolder {
    meta: Arc<Mutex<ObjectArchiveStateMeta>>,
}

impl ObjectArchiveStateMetaHolder {
    pub fn new(id: u64) -> Self {
        let meta = ObjectArchiveStateMeta::new(id);

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
            let mut empty_meta = ObjectArchiveStateMeta::new(meta.id);
            std::mem::swap(meta.deref_mut(), &mut empty_meta);

            empty_meta
        };

        meta
    }
}
