use cyfs_lib::*;
use cyfs_base::*;

use std::{sync::Arc, ops::Deref};

#[derive(Clone)]
pub struct FriendsManager {
    cache: Arc<StateMapViewCache>,
}

impl FriendsManager {
    pub fn new(
        global_state: GlobalStateOutputProcessorRef,
    ) -> Self {
        let state_view = StateView::new(
            global_state,
            CYFS_FRIENDS_LIST_PATH,
            ObjectMapSimpleContentType::Map,
            None,
            Some(cyfs_core::get_system_dec_app().object_id().to_owned()),
        );

        let cache = StateMapViewCache::new(state_view);
        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        if let Err(e) = self.cache.load().await {
            error!("load friends to cache failed! {}", e);
            return Err(e);
        }

        let coll = self.cache.coll().read().unwrap();
        let all: Vec<&String> = coll.keys().collect();
        info!("load friends list success! {:?}", all);

        Ok(())
    }

    async fn start_auto_load(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60 * 10)).await;
    
                this.load().await;
            }
        });
    }

    async fn load(&self) {
        match self.cache.load().await {
            Ok(true) => {
                //let coll = self.cache.coll().read().unwrap();
                let value = serde_json::to_string(&self.cache.coll().read().unwrap().deref()).unwrap();
                info!("friend list updated! {}", value);
            }
            Ok(false) => {
                debug!("load friends list but not changed!")
            }
            Err(e) => {
                info!("load friends list error! {}", e);
            }
        }
    }

    pub fn is_friend(&self, owner: &ObjectId) -> bool {
        self.cache.get(&owner.to_string()).is_some()
    }
}