use super::manager::*;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_core::TransContextObject;

use async_std::sync::Mutex as AsyncMutex;
use std::collections::LinkedList;
use std::sync::Arc;

const CYFS_CONTEXT_OBJECT_EXPIRED_DATE: u64 = 1000 * 1000 * 5;

#[derive(Debug)]
pub(crate) enum TransContextRef {
    Object(ObjectId),
    Path((String, ObjectId)),
}

struct CachedContext {
    context: Option<Arc<ContextItem>>,
    last_updated: u64,
}

struct TransContextHolderInner {
    ref_id: TransContextRef,
    cache: AsyncMutex<CachedContext>,
    referer: String,
    manager: ContextManager,
}

impl TransContextHolderInner {
    pub fn new(
        manager: ContextManager,
        ref_id: TransContextRef,
        referer: impl Into<String>,
    ) -> Self {
        Self {
            manager,
            ref_id,
            cache: AsyncMutex::new(CachedContext {
                context: None,
                last_updated: 0,
            }),
            referer: referer.into(),
        }
    }

    async fn get_context(&self) -> Option<Arc<ContextItem>> {
        let mut cache = self.cache.lock().await;
        if let Some(context) = &cache.context {
            if bucky_time_now() - cache.last_updated < CYFS_CONTEXT_OBJECT_EXPIRED_DATE {
                return Some(context.clone());
            }
        }

        let context = match &self.ref_id {
            TransContextRef::Object(id) => self.manager.get_context(id).await,
            TransContextRef::Path((path, dec_id)) => {
                self.manager.search_context(dec_id, path).await
            }
        };

        cache.context = context.clone();
        cache.last_updated = bucky_time_now();

        context
    }

    async fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        let ret = self.get_context().await;
        if ret.is_none() {
            return false;
        }

        let context = ret.unwrap();
        let ret = context.object.device_list().iter().find(|item| {
            if item.target.eq(target) && item.chunk_codec_type.support_desc(encode_desc) {
                true
            } else {
                false
            }
        });

        ret.is_some()
    }

    async fn sources_of(
        &self,
        filter: Box<dyn Fn(&DownloadSource) -> bool>,
        limit: usize,
    ) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let ret = self.get_context().await;
        if ret.is_none() {
            return result;
        }

        let context = ret.unwrap();
        let mut count = 0;
        for source in &context.source_list {
            if (*filter)(source) {
                result.push_back(source.clone());
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }

        result
    }
}

#[derive(Clone)]
pub(super) struct TransContextHolder(Arc<TransContextHolderInner>);

impl TransContextHolder {
    pub fn new(manager: ContextManager, ref_id: TransContextRef, referer: impl Into<String>) -> Self {
        Self(Arc::new(TransContextHolderInner::new(manager, ref_id, referer)))
    }

    pub async fn init(&self) -> BuckyResult<()> {
        match self.0.get_context().await {
            Some(_) => {
                Ok(())
            }
            None => {
                let msg = format!("trans context not found! context={:?}", self.0.ref_id);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}

impl DownloadContext for TransContextHolder {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn referer(&self) -> &str {
        &self.0.referer
    }

    fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        todo!();
    }

    fn sources_of(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> LinkedList<DownloadSource> {
        todo!();
    }
}
