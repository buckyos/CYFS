use super::data::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};

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
    pub isolates: Vec<ObjectArchiveIsolateMeta>,
    pub roots: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveStateMeta {
    pub fn new() -> Self {
        Self {
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
