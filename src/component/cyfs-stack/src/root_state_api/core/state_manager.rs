use super::global_state::*;
use super::root_index::GlobalRootIndex;
use super::state_list_index::GlobalStateListIndex;
use crate::config::StackGlobalConfig;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Eq, PartialEq, Hash)]
struct GlobalStateKey {
    category: GlobalStateCategory,
    isolate_id: ObjectId,
}

struct GlobalStateItem {
    state: Option<GlobalStateRef>,
}

#[derive(Clone)]
pub struct GlobalStateManager {
    root_state: Arc<AsyncMutex<HashMap<ObjectId, GlobalStateItem>>>,
    local_cache: Arc<AsyncMutex<HashMap<ObjectId, GlobalStateItem>>>,

    index: Arc<GlobalStateListIndex>,
    noc: NamedObjectCacheRef,
    config: StackGlobalConfig,
}

impl GlobalStateManager {
    pub fn new(noc: NamedObjectCacheRef, config: StackGlobalConfig) -> Self {
        Self {
            index: Arc::new(GlobalStateListIndex::new(noc.clone())),
            noc,
            config,
            root_state: Arc::new(AsyncMutex::new(HashMap::new())),
            local_cache: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    pub fn clone_processor(&self) -> GlobalStateManagerRawProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub async fn load(&self) -> BuckyResult<()> {
        self.index.load().await
    }

    fn select_list(
        &self,
        category: GlobalStateCategory,
    ) -> &Arc<AsyncMutex<HashMap<ObjectId, GlobalStateItem>>> {
        match category {
            GlobalStateCategory::RootState => &self.root_state,
            GlobalStateCategory::LocalCache => &self.local_cache,
        }
    }

    pub async fn get_root_state(&self, isolate_id: &ObjectId) -> Option<GlobalStateRef> {
        self.get_global_state(GlobalStateCategory::RootState, isolate_id)
            .await
    }

    pub async fn get_local_cache(&self, isolate_id: &ObjectId) -> Option<GlobalStateRef> {
        self.get_global_state(GlobalStateCategory::LocalCache, isolate_id)
            .await
    }

    pub async fn get_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
    ) -> Option<GlobalStateRef> {
        let list = self.select_list(category).lock().await;
        list.get(isolate_id)
            .map(|item| item.state.clone())
            .flatten()
    }

    pub async fn load_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRef>> {
        let mut list = self.select_list(category).lock().await;
        if let Some(item) = list.get(isolate_id) {
            match &item.state {
                Some(state) => {
                    return Ok(Some(state.clone()));
                }
                None => {
                    if !auto_create {
                        return Ok(None);
                    }
                }
            }
        }

        let state = self
            .load_global_state_impl(category, isolate_id, owner, auto_create)
            .await?;
        let item = GlobalStateItem {
            state: state.clone(),
        };

        if let Some(_) = list.insert(isolate_id.to_owned(), item) {
            unreachable!();
        }

        Ok(state)
    }

    async fn load_global_state_impl(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRef>> {
        if !auto_create {
            if !GlobalRootIndex::exists(category, isolate_id, &self.noc).await? {
                let msg = format!(
                    "load global state but not exists! category={}, isolate={}",
                    category, isolate_id
                );
                warn!("{}", msg);
                return Ok(None);
            }
        }

        let state = GlobalState::load(
            category,
            isolate_id,
            owner.clone(),
            self.noc.clone(),
            self.config.clone(),
        )
        .await?;
        let state = Arc::new(state);

        info!(
            "load global state success! category={}, isolate={}, root={:?}",
            category,
            isolate_id,
            state.get_current_root()
        );

        self.index
            .on_new_global_state(category, isolate_id, owner)
            .await?;

        Ok(Some(state))
    }
}

#[async_trait::async_trait]
impl GlobalStateManagerRawProcessor for GlobalStateManager {
    async fn get_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
    ) -> Option<GlobalStateRawProcessorRef> {
        Self::get_global_state(&self, category, isolate_id)
            .await
            .map(|item| item.clone_processor())
    }

    async fn load_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRawProcessorRef>> {
        Self::load_global_state(&self, category, isolate_id, owner, auto_create)
            .await
            .map(|ret| ret.map(|item| item.clone_processor()))
    }
}
