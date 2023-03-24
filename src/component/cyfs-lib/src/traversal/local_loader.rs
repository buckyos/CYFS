use super::traverser::*;
use crate::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_util::AsyncReadWithSeek;

use std::sync::Arc;

pub struct ObjectTraverserLocalLoader {
    noc: NamedObjectCacheRef,
    chunk_store: ChunkReaderRef,
}

impl ObjectTraverserLocalLoader {
    pub fn new(noc: NamedObjectCacheRef, chunk_store: ChunkReaderRef) -> Self {
        Self { noc, chunk_store }
    }

    pub fn into_reader(self) -> ObjectTraverserLoaderRef {
        Arc::new(Box::new(self))
    }
}

#[async_trait::async_trait]
impl ObjectTraverserLoader for ObjectTraverserLocalLoader {
    async fn get_object(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectTraverserLoaderObjectData>> {
        let mut req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
            flags: 0,
        };
        req.set_no_update_last_access();
        
        match self.noc.get_object(&req).await {
            Ok(Some(data)) => {
                let ret = ObjectTraverserLoaderObjectData {
                    object: data.object,
                    meta: Some(data.meta),
                };

                Ok(Some(ret))
            }
            Ok(None) => {
                warn!("traverser get object from noc but not found! {}", object_id);
                Ok(None)
            }
            Err(e) => {
                error!("traverser get object from noc error! {}, {}", object_id, e);
                Err(e)
            }
        }
    }

    async fn get_chunk(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>>> {
        match self.chunk_store.get(chunk_id).await {
            Ok(chunk) => Ok(Some(chunk)),
            Err(e) if e.code() == BuckyErrorCode::NotFound => {
                warn!(
                    "traverser get chunk from chunk store but not found! {}",
                    chunk_id
                );
                Ok(None)
            }
            Err(e) => {
                error!(
                    "traverser get chunk from chunk store error! {}, {}",
                    chunk_id, e
                );
                Err(e)
            }
        }
    }
}
