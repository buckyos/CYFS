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

struct ContextValue {
    ref_id: TransContextRef,
    cache: AsyncMutex<CachedContext>,
}

struct TargetValue {
    target: DeviceId,
    source: DownloadSource<DeviceDesc>,
}

enum TransContentValue {
    Target(TargetValue),
    Context(ContextValue),
}

struct TransContextHolderInner {
    value: TransContentValue,
    referer: String,
    manager: ContextManager,
}

impl TransContextHolderInner {
    pub fn new_context(
        manager: ContextManager,
        ref_id: TransContextRef,
        referer: impl Into<String>,
    ) -> Self {
        let value = ContextValue {
            ref_id,
            cache: AsyncMutex::new(CachedContext {
                context: None,
                last_updated: 0,
            }),
        };

        Self {
            manager,
            value: TransContentValue::Context(value),
            referer: referer.into(),
        }
    }

    pub fn new_target(
        manager: ContextManager,
        target: DeviceId,
        target_desc: DeviceDesc,
        referer: impl Into<String>,
    ) -> Self {
        let source = DownloadSource {
            target: target_desc,
            codec_desc: ChunkCodecDesc::Stream(None, None, None),
        };

        let value = TargetValue { target, source };

        Self {
            manager,
            value: TransContentValue::Target(value),
            referer: referer.into(),
        }
    }

    fn target_value(&self) -> &TargetValue {
        match &self.value {
            TransContentValue::Target(v) => v,
            _ => unreachable!(),
        }
    }

    fn context_value(&self) -> &ContextValue {
        match &self.value {
            TransContentValue::Context(v) => v,
            _ => unreachable!(),
        }
    }

    async fn get_context(&self) -> Option<Arc<ContextItem>> {
        let mut cache = self.context_value().cache.lock().await;
        if let Some(context) = &cache.context {
            if bucky_time_now() - cache.last_updated < CYFS_CONTEXT_OBJECT_EXPIRED_DATE {
                return Some(context.clone());
            }
        }

        let context = match &self.context_value().ref_id {
            TransContextRef::Object(id) => self.manager.get_context(id).await,
            TransContextRef::Path((path, dec_id)) => {
                self.manager.search_context(dec_id, path).await
            }
        };

        cache.context = context.clone();
        cache.last_updated = bucky_time_now();

        context
    }

    async fn source_exists_with_context(
        &self,
        target: &DeviceId,
        codec_desc: &ChunkCodecDesc,
    ) -> bool {
        let ret = self.get_context().await;
        if ret.is_none() {
            return false;
        }

        let context = ret.unwrap();
        let ret = context.object.device_list().iter().find(|item| {
            if item.target.eq(target) && item.chunk_codec_desc.support_desc(codec_desc) {
                true
            } else {
                false
            }
        });

        ret.is_some()
    }

    async fn sources_of_with_context(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> LinkedList<DownloadSource<DeviceDesc>> {
        let mut result = LinkedList::new();
        let ret = self.get_context().await;
        if ret.is_none() {
            return result;
        }

        let context = ret.unwrap();
        let mut count = 0;
        for source in &context.source_list {
            if filter.check(source) {
                result.push_back(source.clone());
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }

        result
    }

    async fn source_exists_with_target(
        &self,
        target: &DeviceId,
        codec_desc: &ChunkCodecDesc,
    ) -> bool {
        let value = self.target_value();
        value.target.eq(target) && value.source.codec_desc.support_desc(&codec_desc)
    }

    async fn sources_of_with_target(
        &self,
        filter: &DownloadSourceFilter,
        _limit: usize,
    ) -> LinkedList<DownloadSource<DeviceDesc>> {
        let mut result = LinkedList::new();
        let value = self.target_value();
        if filter.check(&value.source) {
            result.push_back(value.source.clone());
        }
        result
    }

    async fn source_exists(&self, target: &DeviceId, codec_desc: &ChunkCodecDesc) -> bool {
        match &self.value {
            TransContentValue::Target(_) => {
                self.source_exists_with_target(target, codec_desc).await
            }
            TransContentValue::Context(_) => {
                self.source_exists_with_context(target, codec_desc).await
            }
        }
    }

    async fn sources_of(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> LinkedList<DownloadSource<DeviceDesc>> {
        match &self.value {
            TransContentValue::Target(_) => self.sources_of_with_target(filter, limit).await,
            TransContentValue::Context(_) => self.sources_of_with_context(filter, limit).await,
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        match &self.value {
            TransContentValue::Target(_) => Ok(()),
            TransContentValue::Context(context) => match self.get_context().await {
                Some(_) => Ok(()),
                None => {
                    let msg = format!("trans context not found! context={:?}", context.ref_id);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            },
        }
    }

    async fn non_target(&self) -> Option<DeviceId> {
        match &self.value {
            TransContentValue::Target(value) => Some(value.target.clone()),
            TransContentValue::Context(value) => {
                match self.get_context().await {
                    Some(context) => {
                        if context.source_list.len() > 0 {
                            Some(context.source_list[0].target.device_id())
                        } else {
                            warn!("trans context for non target but source list is empty! context={:?}", value.ref_id);
                            None
                        }
                    }
                    None => {
                        warn!(
                            "trans context for non target not found! context={:?}",
                            value.ref_id
                        );
                        None
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct TransContextHolder(Arc<TransContextHolderInner>);

impl TransContextHolder {
    pub fn new_context(
        manager: ContextManager,
        ref_id: TransContextRef,
        referer: impl Into<String>,
    ) -> Self {
        Self(Arc::new(TransContextHolderInner::new_context(
            manager, ref_id, referer,
        )))
    }

    pub fn new_target(
        manager: ContextManager,
        target: DeviceId,
        target_desc: DeviceDesc,
        referer: impl Into<String>,
    ) -> Self {
        Self(Arc::new(TransContextHolderInner::new_target(
            manager,
            target,
            target_desc,
            referer,
        )))
    }

    pub async fn init(&self) -> BuckyResult<()> {
        self.0.init().await
    }

    pub async fn non_target(&self) -> Option<DeviceId> {
        self.0.non_target().await
    }
}

#[async_trait::async_trait]
impl DownloadContext for TransContextHolder {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn referer(&self) -> &str {
        &self.0.referer
    }

    async fn source_exists(&self, source: &DownloadSource<DeviceId>) -> bool {
        self.0
            .source_exists(&source.target, &source.codec_desc)
            .await
    }

    async fn sources_of(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> LinkedList<DownloadSource<DeviceDesc>> {
        self.0.sources_of(filter, limit).await
    }
}