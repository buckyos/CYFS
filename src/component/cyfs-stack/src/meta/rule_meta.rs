use super::meta_cache::*;
use super::raw_meta::*;
use cyfs_base::{BuckyResult, ObjectId, NameInfo, NameState};

use async_trait::async_trait;
#[derive(Clone)]
pub(crate) struct MetaCacheWithRule {
    raw_meta_cache: RawMetaCache,
}

impl MetaCacheWithRule {
    pub fn new(raw_meta_cache: RawMetaCache) -> Self {
        Self {
            raw_meta_cache,
        }
    }

    pub async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<MetaObjectCacheData>> {
        // TODO 事件支持

        self.raw_meta_cache.get_object(object_id).await
    }

    pub async fn flush_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        // TODO 事件支持

        self.raw_meta_cache.flush_object(object_id).await
    }

    async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        // TODO 事件支持

        self.raw_meta_cache.get_name(name).await
    }
}

#[async_trait]
impl MetaCache for MetaCacheWithRule {
    async fn get_object(&self, object_id: &ObjectId) -> BuckyResult<Option<MetaObjectCacheData>> {
        MetaCacheWithRule::get_object(&self, object_id).await
    }

    async fn flush_object(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        MetaCacheWithRule::flush_object(&self, object_id).await
    }

    async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        MetaCacheWithRule::get_name(&self, name).await
    }

    fn clone_meta(&self) -> Box<dyn MetaCache> {
        Box::new(self.clone()) as Box<dyn MetaCache>
    }
}
