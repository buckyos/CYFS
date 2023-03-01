use cyfs_base::*;
use super::traverser::ObjectTraverserLoaderRef;

use std::sync::Arc;

pub struct ObjectMapNOCCacheTranverseAdapter {
    loader: ObjectTraverserLoaderRef,
}

impl ObjectMapNOCCacheTranverseAdapter {
    pub fn new(loader: ObjectTraverserLoaderRef) -> Self {
        Self { loader }
    }

    pub fn new_noc_cache(loader: ObjectTraverserLoaderRef) -> ObjectMapNOCCacheRef {
        let ret = Self::new(loader);
        Arc::new(Box::new(ret) as Box<dyn ObjectMapNOCCache>)
    }
}

#[async_trait::async_trait]
impl ObjectMapNOCCache for ObjectMapNOCCacheTranverseAdapter {
    async fn exists(&self, _dec_id: Option<ObjectId>, _object_id: &ObjectId) -> BuckyResult<bool> {
        unimplemented!();
    }

    async fn get_object_map_ex(
        &self,
        _dec: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectMapCacheItem>> {

        let resp = self.loader.get_object(&object_id).await?;

        match resp {
            Some(resp) => {
                match ObjectMap::raw_decode(&resp.object.object_raw) {
                    Ok((object, _)) => {
                        // 首次加载后，直接设置id缓存，减少一次id计算
                        object.direct_set_object_id_on_init(object_id);

                        let access = match resp.meta {
                            Some(meta) => meta.access_string,
                            None => 0,
                        };

                        let item = ObjectMapCacheItem {
                            object,
                            access: AccessString::new(access),
                        };
                        Ok(Some(item))
                    }
                    Err(e) => {
                        error!("decode ObjectMap object error: id={}, {}", object_id, e);
                        Err(e)
                    }
                }
            }
            None => Ok(None),
        }
    }

    async fn put_object_map(
        &self,
        _dec_id: Option<ObjectId>,
        _object_id: ObjectId,
        _object: ObjectMap,
        _access: Option<AccessString>,
    ) -> BuckyResult<()> {
        unimplemented!();
    }
}
