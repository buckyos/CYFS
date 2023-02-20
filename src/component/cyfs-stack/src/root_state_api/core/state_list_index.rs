use cyfs_base::*;
use cyfs_lib::*;

use async_std::sync::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalStateMetaInfo {
    owner: Option<ObjectId>,
    create_time: u64,
    category: Vec<GlobalStateCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalStateMetaInfoList {
    list: HashMap<ObjectId, GlobalStateMetaInfo>,
}

impl Default for GlobalStateMetaInfoList {
    fn default() -> Self {
        Self {
            list: HashMap::new(),
        }
    }
}

declare_collection_codec_for_serde!(GlobalStateMetaInfoList);

pub(super) struct GlobalStateListIndex {
    list: RwLock<GlobalStateMetaInfoList>,
    storage: NOCStorageWrapper,
}

impl GlobalStateListIndex {
    pub fn new(noc: NamedObjectCacheRef) -> Self {
        let id = "cyfs-global-state-list";

        let storage = NOCStorageWrapper::new(&id, noc);

        Self {
            list: RwLock::new(GlobalStateMetaInfoList::default()),
            storage,
        }
    }

    pub async fn get_isolate_list(&self, category: GlobalStateCategory,) -> Vec<GlobalStateIsolateInfo> {
        let list = self.list.read().await;
        list.list.iter().filter_map(|(id, item)| {
            if item.category.contains(&category) {
                Some(GlobalStateIsolateInfo {
                    isolate_id: id.to_owned(),
                    owner: item.owner.clone(),
                    create_time: item.create_time,
                })
            } else {
                None
            }            
        }).collect()
    }

    pub async fn load(&self) -> BuckyResult<()> {
        let value: Option<GlobalStateMetaInfoList> = self.storage.load().await.map_err(|e| {
            error!("load global state list from noc error! {}", e);
            e
        })?;

        info!("load global state list success! {:?}", value,);

        match value {
            Some(new_list) => {
                let mut list = self.list.write().await;
                assert!(list.list.is_empty());
                *list = new_list;
            }
            None => {}
        }

        Ok(())
    }

    pub async fn on_new_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
    ) -> BuckyResult<()> {
        let mut list = self.list.write().await;

        let prev = list.clone();
        match list.list.entry(isolate_id.to_owned()) {
            Entry::Occupied(mut o) => {
                let current = o.get_mut();
                if current.category.contains(&category) {
                    debug!(
                        "save new global state to noc but already exists! category={}, isolate={}",
                        category, isolate_id
                    );
            
                    return Ok(());
                }

                current.category.push(category);
            }
            Entry::Vacant(v) => {
                let item = GlobalStateMetaInfo {
                    owner,
                    create_time: bucky_time_now(),
                    category: vec![category],
                };

                v.insert(item);
            }
        }

        self.storage.save(&*list).await.map_err(|e| {
            error!(
                "save global state to noc failed! category={}, isolate={}, {}",
                category, isolate_id, e
            );

            *list = prev;
            e
        })?;

        info!(
            "save new global state to noc success! category={}, isolate={}",
            category, isolate_id
        );

        Ok(())
    }
}
