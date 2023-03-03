use super::log::BackupLogManager;
use super::writer::*;
use crate::archive::*;
use crate::object_pack::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::AsyncReadWithSeek;

use std::sync::{Arc, Mutex};

pub struct BackupDataStatWriterInner {
    meta: ObjectArchiveMeta,
    completed: ObjectArchiveDataMetas,
}

impl BackupDataStatWriterInner {
    pub fn new(id: u64, format: ObjectPackFormat) -> Self {
        let meta = ObjectArchiveMeta::new(id, format);
        Self {
            meta,
            completed: ObjectArchiveDataMetas::default(),
        }
    }

    pub fn add_isolate_meta(&mut self, isolate_meta: ObjectArchiveIsolateMeta) {
        self.meta.add_isolate(isolate_meta);
    }

    pub fn add_object(&mut self, bytes: usize) {
        self.completed.objects.bytes += bytes as u64;
        self.completed.objects.count += 1;
    }

    pub fn add_chunk(&mut self, bytes: usize) {
        self.completed.chunks.bytes += bytes as u64;
        self.completed.chunks.count += 1;
    }

    pub fn finish(&mut self) -> ObjectArchiveMeta {
        let mut empty_meta = ObjectArchiveMeta::new(self.meta.id, self.meta.format);
        std::mem::swap(&mut self.meta, &mut empty_meta);

        empty_meta
    }
}

#[derive(Clone)]
pub struct BackupDataStatWriter(Arc<Mutex<BackupDataStatWriterInner>>);

impl BackupDataStatWriter {
    pub fn new(id: u64, format: ObjectPackFormat) -> Self {
        let inner = BackupDataStatWriterInner::new(id, format);
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn into_writer(self) -> BackupDataWriterRef {
        Arc::new(Box::new(self))
    }

    pub async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        let mut inner = self.0.lock().unwrap();
        inner.add_isolate_meta(isolate_meta);
    }

    pub async fn add_object(
        &self,
        _object_id: &ObjectId,
        object_raw: &[u8],
        _meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        let bytes = object_raw.len();

        let mut inner = self.0.lock().unwrap();
        inner.add_object(bytes);

        Ok(())
    }

    pub async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        _data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        _meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        let bytes = chunk_id.len();

        let mut inner = self.0.lock().unwrap();
        inner.add_chunk(bytes);

        Ok(())
    }

    pub async fn finish(&self) -> BuckyResult<ObjectArchiveMeta> {
        let mut inner = self.0.lock().unwrap();

        Ok(inner.finish())
    }
}

#[async_trait::async_trait]
impl BackupDataWriter for BackupDataStatWriter {
    async fn add_isolate_meta(&self, isolate_meta: ObjectArchiveIsolateMeta) {
        Self::add_isolate_meta(&self, isolate_meta).await
    }

    async fn add_object(
        &self,
        object_id: &ObjectId,
        object_raw: &[u8],
        meta: Option<&NamedObjectMetaData>,
    ) -> BuckyResult<()> {
        Self::add_object(&self, object_id, object_raw, meta).await
    }

    async fn add_chunk(
        &self,
        chunk_id: ChunkId,
        data: Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>,
        meta: Option<ArchiveInnerFileMeta>,
    ) -> BuckyResult<()> {
        Self::add_chunk(&self, chunk_id, data, meta).await
    }

    fn logger(&self) -> Option<&BackupLogManager> {
        None
    }

    async fn finish(&self) -> BuckyResult<ObjectArchiveMeta> {
        Self::finish(&self).await
    }
}
