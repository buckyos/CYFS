use std::path::Path;
use async_trait::async_trait;


use crate::{archive_storage::ArchiveRef};
use async_std::sync::{Arc, MutexGuard};

pub type ArchiveStorageRef = Arc<Box<dyn ArchiveStorage>>;

#[async_trait]
pub trait ArchiveStorage: std::marker::Send + Sync {
    fn path(&self) -> &Path;

    async fn create_archive(&self, read_only: bool) -> &ArchiveRef;

    async fn get_locker(&self) -> MutexGuard<'_, ()>;
}

