mod local;
mod meta;

pub use local::*;
pub use meta::*;


use cyfs_base::BuckyResult;

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait DeviceConfigRepo: Send + Sync {
    async fn fetch(&self) -> BuckyResult<String>;

    fn get_type(&self) -> &'static str;
}

pub type DeviceConfigRepoRef = Arc<Box<dyn DeviceConfigRepo>>;