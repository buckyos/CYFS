use cyfs_base::{AnyNamedObject, BuckyResult, ObjectId, NameInfo, NameState};

use async_trait::async_trait;
use std::sync::Arc;

pub struct MetaObjectCacheData {
    // 对象内容
    pub object_raw: Vec<u8>,
    pub object: Arc<AnyNamedObject>,
}

#[async_trait]
pub trait MetaCache: Sync + Send {
    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<MetaObjectCacheData>>;

    async fn flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool>;

    async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>>;

    fn clone_meta(&self) -> Box<dyn MetaCache>;
}
