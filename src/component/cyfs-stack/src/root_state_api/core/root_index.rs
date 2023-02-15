use super::revision::*;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RootInfo {
    pub root_state: Option<ObjectId>,
    pub revision: u64,
}

impl Default for RootInfo {
    fn default() -> Self {
        Self {
            root_state: None,
            revision: 0,
        }
    }
}

declare_collection_codec_for_serde!(RootInfo);

pub(crate) struct GlobalRootIndex {
    category: GlobalStateCategory,

    // 只有在持有update_lock情况下，才可以更新root
    root: RwLock<RootInfo>,
    update_lock: AsyncMutex<()>,
    storage: NOCStorageWrapper,

    revision: RevisionList,
}

impl GlobalRootIndex {
    pub fn new(
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        noc: NamedObjectCacheRef,
        revision: RevisionList,
    ) -> Self {
        let id = Self::make_id(category, isolate_id);

        Self {
            category,
            root: RwLock::new(RootInfo::default()),
            update_lock: AsyncMutex::new(()),
            storage: NOCStorageWrapper::new(&id, noc),
            revision,
        }
    }

    fn make_id(category: GlobalStateCategory, isolate_id: &ObjectId) -> String {
        match category {
            GlobalStateCategory::RootState => {
                format!("cyfs-global-root-state-{}", isolate_id.to_string())
            }
            GlobalStateCategory::LocalCache => {
                format!("cyfs-global-local-cache-{}", isolate_id.to_string())
            }
        }
    }

    pub async fn exists(
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        noc: &NamedObjectCacheRef,
    ) -> BuckyResult<bool> {
        let id = Self::make_id(category, isolate_id);
        NOCStorageWrapper::exists(&id, noc).await
    }

    pub async fn load(&self) -> BuckyResult<()> {
        let value: Option<RootInfo> = self.storage.load().await.map_err(|e| {
            error!(
                "load global root from noc error! category={}, {}",
                self.category, e
            );
            e
        })?;

        info!(
            "load global root success! category={}, {:?}",
            self.category, value
        );

        let _update_lock = self.update_lock.lock().await;
        let mut root_info = self.root.write().unwrap();
        match value {
            Some(info) => {
                // 如果加载到了有效root，那么需要立即更新revision->global_root的映射关系
                if let Some(root) = &info.root_state {
                    assert!(info.revision > 0);
                    self.revision
                        .insert_revision(info.revision, root.to_owned());
                }

                *root_info = info;
            }
            None => {}
        }

        Ok(())
    }

    // direct set the root and revision, always during the zone sync requests
    pub(crate) async fn direct_set_root_state(
        &self,
        new_root_info: RootInfo,
        prev_root_id: Option<ObjectId>,
    ) -> BuckyResult<()> {
        assert!(new_root_info.root_state.is_some());

        let _update_lock = self.update_lock.lock().await;
        let prev_root_info;
        {
            let mut root_info = self.root.write().unwrap();
            if prev_root_id.is_some() {
                if root_info.root_state != prev_root_id {
                    let msg = format!(
                        "direct set global state root but unmatch! category={}, prev={:?}, current={:?}, new={:?}",
                        self.category, prev_root_id, root_info.root_state, new_root_info
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            }

            info!(
                "direct set global state root! category={}, current={:?}, new={:?}",
                self.category, root_info, new_root_info
            );
            prev_root_info = root_info.clone();
            *root_info = new_root_info.clone();
        }

        // 需要在写锁里面发起保存
        self.storage.save(&new_root_info).await.map_err(|e| {
            let mut root_info = self.root.write().unwrap();
            error!(
                "save global state root to noc failed! category={}, state={:?}, {}",
                self.category, root_info.root_state, e
            );

            // FIXME 保存失败，这里是否需要回滚？
            *root_info = prev_root_info;

            e
        })?;

        // 保存revision->root的映射
        self.revision
            .insert_revision(new_root_info.revision, new_root_info.root_state.unwrap());

        Ok(())
    }

    pub fn get_root_state(&self) -> RootInfo {
        let root = self.root.read().unwrap();
        root.clone()
    }

    pub async fn update_root_state(
        &self,
        new_root_id: ObjectId,
        prev_root_id: Option<ObjectId>,
    ) -> BuckyResult<()> {
        info!(
            "will update global root: category={}, {:?} -> {}",
            self.category, prev_root_id, new_root_id
        );

        let _update_lock = self.update_lock.lock().await;
        let current_root_info;
        {
            let mut root_info = self.root.write().unwrap();
            if root_info.root_state != prev_root_id {
                let msg = format!(
                    "update global root but unmatch! category={}, prev={:?}, current={:?}, new={}",
                    self.category, prev_root_id, root_info.root_state, new_root_id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            root_info.root_state = Some(new_root_id);
            root_info.revision += 1;
            current_root_info = root_info.clone();
        }

        // 需要在写锁里面发起保存
        self.storage.save(&current_root_info).await.map_err(|e| {
            let mut root_info = self.root.write().unwrap();
            error!(
                "save global root to noc failed! category={}, state={:?}, {}",
                self.category, root_info.root_state, e
            );

            // FIXME 保存失败，这里是否需要回滚？
            root_info.root_state = prev_root_id;
            root_info.revision -= 1;

            e
        })?;

        // 保存revision->root的映射
        self.revision.insert_revision(
            current_root_info.revision,
            current_root_info.root_state.unwrap(),
        );

        Ok(())
    }
}

pub(crate) type GlobalRootIndexRef = Arc<GlobalRootIndex>;
