use crate::archive::ObjectArchiveInnerFileData;
use cyfs_base::*;

use std::path::Path;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ObjectRestorer: Send + Sync {
    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()>;
}

pub type ObjectRestorerRef = Arc<Box<dyn ObjectRestorer>>;