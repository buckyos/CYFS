mod file_storage;
mod local_store;
mod sqlite_storage;

pub use file_storage::FileStorage;
pub use local_store::LocalStore;
pub use sqlite_storage::SqliteStorage;

use async_std::sync::{Arc, Mutex};
use async_trait::async_trait;
use cyfs_base::BuckyError;

#[async_trait]
pub trait AsyncStorage: Send + Sync {
    async fn set_item(&mut self, key: &str, value: String) -> Result<(), BuckyError>;

    async fn get_item(&self, key: &str) -> Option<String>;

    async fn remove_item(&mut self, key: &str) -> Option<()>;

    async fn clear(&mut self);

    async fn clear_with_prefix(&mut self, prefix: &str);
}

pub type AsyncStorageSync = Arc<Mutex<Box<dyn AsyncStorage>>>;

pub fn into_async_storage_sync<T>(storage: T) -> AsyncStorageSync
where
    T: AsyncStorage + 'static,
{
    Arc::new(Mutex::new(Box::new(storage) as Box<dyn AsyncStorage>))
}
