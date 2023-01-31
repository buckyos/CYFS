use super::manager::*;
use super::state::{ContextSourceDownloadStateManager, NDNTaskCancelStrategy};
use cyfs_base::*;
use cyfs_bdt::ndn::channel::DownloadSession;
use cyfs_bdt::*;
use cyfs_core::TransContextObject;

use async_std::sync::Mutex as AsyncMutex;
use std::collections::LinkedList;
use std::sync::Arc;

const CYFS_CONTEXT_OBJECT_EXPIRED_DATE: u64 = 1000 * 1000 * 10;

#[derive(Debug)]
pub enum TransContextRef {
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
    manager: Option<ContextManager>,
    state: ContextSourceDownloadStateManager,
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
            manager: Some(manager),
            value: TransContentValue::Context(value),
            referer: referer.into(),
            state: ContextSourceDownloadStateManager::new(NDNTaskCancelStrategy::WaitingSource),
        }
    }

    pub fn new_target(
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
            manager: None,
            value: TransContentValue::Target(value),
            referer: referer.into(),
            state: ContextSourceDownloadStateManager::new(NDNTaskCancelStrategy::AutoCancel),
        }
    }

    pub fn debug_string(&self) -> String {
        match &self.value {
            TransContentValue::Target(v) => {
                format!("target={}", v.target)
            }
            TransContentValue::Context(v) => {
                format!("context={:?}", v.ref_id)
            }
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

        let manager = self.manager.as_ref().unwrap();
        let context = match &self.context_value().ref_id {
            TransContextRef::Object(id) => manager.get_context(id).await,
            TransContextRef::Path((path, dec_id)) => {
                if path.starts_with('$') {
                    manager.search_context(None, path).await
                } else {
                    manager.search_context(Some(dec_id), path).await
                }
            }
        };

        // output some debug infos
        if let Some(old) = &cache.context {
            if let Some(new) = &context {
                if old.object_id != new.object_id {
                    warn!(
                        "context changed! context={}, {} -> {}, devices={:?}",
                        self.debug_string(),
                        old.object_id,
                        new.object_id,
                        new.object.device_list(),
                    );
                } else {
                    if old.object.device_list() != new.object.device_list() {
                        warn!(
                            "context device list changed! context={}, devices: {:?} -> {:?}",
                            self.debug_string(),
                            old.object.device_list(),
                            new.object.device_list(),
                        );
                    }
                }
            } else {
                warn!(
                    "context changed to none! context={}, {} -> None",
                    self.debug_string(),
                    old.object_id
                );
            }
        } else {
            if let Some(new) = &context {
                warn!(
                    "context changed! context={}, None -> {}, devices={:?}",
                    self.debug_string(),
                    new.object_id,
                    new.object.device_list()
                );
            }
        }

        cache.context = context.clone();
        cache.last_updated = bucky_time_now();

        context
    }

    async fn sources_of_with_context(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        let mut result = LinkedList::new();
        let ret = self.get_context().await;
        if ret.is_none() {
            return (result, 0);
        }

        let context = ret.unwrap();
        let ts = context
            .object
            .body_expect("context object should has body!")
            .update_time();
        let mut count = 0;
        for source in &context.source_list {
            if filter.check(source) {
                result.push_back(source.clone());
                count += 1;
                if count >= limit {
                    return (result, ts);
                }
            }
        }

        (result, ts)
    }

    async fn sources_of_with_target(
        &self,
        filter: &DownloadSourceFilter,
        _limit: usize,
    ) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        let mut result = LinkedList::new();
        let value = self.target_value();
        if filter.check(&value.source) {
            result.push_back(value.source.clone());
        }

        (result, 0)
    }

    async fn sources_of(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        match &self.value {
            TransContentValue::Target(_) => self.sources_of_with_target(filter, limit).await,
            TransContentValue::Context(_) => self.sources_of_with_context(filter, limit).await,
        }
    }

    async fn update_at(&self) -> Timestamp {
        match &self.value {
            TransContentValue::Target(_) => 0,
            TransContentValue::Context(_) => {
                let ret = self.get_context().await;
                if ret.is_none() {
                    return 0;
                }

                let context = ret.unwrap();
                let ts = context
                    .object
                    .body_expect("context object should has body!")
                    .update_time();

                ts
            }
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
pub struct TransContextHolder(Arc<TransContextHolderInner>);

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
        target: DeviceId,
        target_desc: DeviceDesc,
        referer: impl Into<String>,
    ) -> Self {
        Self(Arc::new(TransContextHolderInner::new_target(
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

    pub fn debug_string(&self) -> String {
        self.0.debug_string()
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

    async fn update_at(&self) -> Timestamp {
        self.0.update_at().await
    }

    async fn sources_of(
        &self,
        filter: &DownloadSourceFilter,
        limit: usize,
    ) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        self.0.sources_of(filter, limit).await
    }

    fn on_new_session(
        &self,
        task: &dyn LeafDownloadTask,
        session: &DownloadSession,
        update_at: Timestamp,
    ) {
        self.0.state.on_new_session(task, session, update_at);
    }

    fn on_drain(&self, task: &dyn LeafDownloadTask, when: Timestamp) {
        self.0.state.on_drain(task, when);
    }
}
