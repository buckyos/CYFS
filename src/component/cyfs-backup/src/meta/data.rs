use cyfs_base::*;

use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDataMeta {
    pub count: u64,
    pub bytes: u64,
}

impl Default for ObjectArchiveDataMeta {
    fn default() -> Self {
        Self { count: 0, bytes: 0 }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDataMetas {
    pub objects: ObjectArchiveDataMeta,
    pub chunks: ObjectArchiveDataMeta,
}

impl Default for ObjectArchiveDataMetas {
    fn default() -> Self {
        Self {
            objects: ObjectArchiveDataMeta::default(),
            chunks: ObjectArchiveDataMeta::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectArchiveDataSeriesMeta {
    pub data: ObjectArchiveDataMetas,
    pub missing: ObjectArchiveDataMetas,
    pub error: ObjectArchiveDataMetas,
}

impl Default for ObjectArchiveDataSeriesMeta {
    fn default() -> Self {
        Self {
            data: ObjectArchiveDataMetas::default(),
            missing: ObjectArchiveDataMetas::default(),
            error: ObjectArchiveDataMetas::default(),
        }
    }
}

impl ObjectArchiveDataSeriesMeta {
    pub fn on_error(&mut self, id: &ObjectId) {
        if id.is_chunk_id() {
            let chunk_id = id.as_chunk_id();
            self.error.chunks.bytes += chunk_id.len() as u64;
            self.error.chunks.count += 1;
        } else {
            self.error.objects.count += 1;
        }
    }

    pub fn on_missing(&mut self, id: &ObjectId) {
        if id.is_chunk_id() {
            let chunk_id = id.as_chunk_id();
            self.missing.chunks.bytes += chunk_id.len() as u64;
            self.missing.chunks.count += 1;
        } else {
            self.missing.objects.count += 1;
        }
    }

    pub fn on_object(&mut self, bytes: usize) {
        self.data.objects.count += 1;
        self.data.objects.bytes += bytes as u64;
    }

    pub fn on_chunk(&mut self, chunk_id: &ChunkId) {
        self.data.chunks.bytes += chunk_id.len() as u64;
        self.data.chunks.count += 1;
    }
}