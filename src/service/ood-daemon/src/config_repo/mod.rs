mod local;
mod meta;
mod http;

#[cfg(test)]
mod test;

pub use local::*;
pub use meta::*;
pub use http::*;

use cyfs_base::BuckyResult;

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait DeviceConfigRepo: Send + Sync {
    async fn fetch(&self) -> BuckyResult<String>;
    async fn clear_cache(&self) {
        // do nothing on default!
    }

    fn get_type(&self) -> &'static str;
}

pub type DeviceConfigRepoRef = Arc<Box<dyn DeviceConfigRepo>>;