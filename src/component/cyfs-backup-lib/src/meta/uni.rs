use super::data::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectArchiveUniMeta {
    pub meta: ObjectArchiveDataSeriesMeta,
}

impl ObjectArchiveUniMeta {
    pub fn new() -> Self {
        Self {
            meta: ObjectArchiveDataSeriesMeta::default(),
        }
    }
}