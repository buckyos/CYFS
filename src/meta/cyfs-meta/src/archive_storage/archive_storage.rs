use std::path::Path;
use async_trait::async_trait;


use crate::{archive_storage::ArchiveRef};
use async_std::sync::{Arc, MutexGuard};

pub fn storage_in_mem_path() -> &'static Path {
    static STORAGE_IN_MEM: &str = "inmemory";
    Path::new(STORAGE_IN_MEM)
}

pub type ArchiveStorageRef = Arc<Box<dyn ArchiveStorage>>;

#[async_trait]
pub trait ArchiveStorage: std::marker::Send + Sync {
    fn path(&self) -> &Path;

    async fn create_archive(&self, read_only: bool) -> ArchiveRef;

    async fn get_locker(&self) -> MutexGuard<'_, ()>;
}

