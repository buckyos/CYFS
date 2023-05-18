use std::time::Duration;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_base_meta::SavedMetaObject;
use cyfs_meta_lib::MetaClient;
use futures::FutureExt;

const TIMEOUT_HALF: Duration = Duration::from_millis(2000);

#[async_trait::async_trait]
pub trait MetaClientTimeout {
    async fn get_desc_timeout(&self, id: &ObjectId) -> BuckyResult<SavedMetaObject>;
}

#[async_trait::async_trait]
impl MetaClientTimeout for MetaClient {
    async fn get_desc_timeout(&self, id: &ObjectId) -> BuckyResult<SavedMetaObject> {
        let fut1 = match futures::future::select(
            self.get_desc(id).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, futures::future::pending::<()>()).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => return ret,
            futures::future::Either::Right((_, fut)) => fut,
        };

        log::warn!("get desc timeout (id={})", id,);

        match futures::future::select(
            self.get_desc(id).boxed(),
            async_std::future::timeout(TIMEOUT_HALF, fut1).boxed(),
        )
        .await
        {
            futures::future::Either::Left((ret, _)) => ret,
            futures::future::Either::Right((ret, _)) => ret.map_or(
                Err(BuckyError::new(BuckyErrorCode::Timeout, "timeout")),
                |ret| ret,
            ),
        }
    }
}
