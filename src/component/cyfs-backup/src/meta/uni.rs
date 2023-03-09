use super::data::*;

use serde::{Deserialize, Serialize};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveUniMeta {
    pub id: u64,
    pub time: String,

    pub meta: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveUniMeta {
    pub fn new(id: u64) -> Self {
        let datetime = chrono::offset::Local::now();
        let time = format!("{:?}", datetime);

        Self {
            id,
            time,
            meta: ObjectArchiveDataSeriesMeta::default(),
        }
    }
}

#[derive(Clone)]
pub struct ObjectArchiveUniMetaHolder {
    meta: Arc<Mutex<ObjectArchiveUniMeta>>,
}

impl ObjectArchiveUniMetaHolder {
    pub fn new(id: u64) -> Self {
        let meta = ObjectArchiveUniMeta::new(id);

        Self {
            meta: Arc::new(Mutex::new(meta)),
        }
    }

    pub fn finish(&self) -> ObjectArchiveUniMeta {
        let meta = {
            let mut meta = self.meta.lock().unwrap();
            let mut empty_meta = ObjectArchiveUniMeta::new(meta.id);
            std::mem::swap(meta.deref_mut(), &mut empty_meta);

            empty_meta
        };

        meta
    }
}
