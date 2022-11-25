use crate::meta::*;
use crate::name::*;
use cyfs_base::*;
use cyfs_lib::*;

use cyfs_util::SNDirParser;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
enum SNConfigListSource {
    Meta,
    Custom,
}

impl Default for SNConfigListSource {
    fn default() -> Self {
        Self::Meta
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SNConfig {
    source: SNConfigListSource,

    // sn list in dir pack mode
    sn: Option<ObjectId>,
}

impl Default for SNConfig {
    fn default() -> Self {
        Self {
            source: SNConfigListSource::default(),
            sn: None,
        }
    }
}

declare_collection_codec_for_serde!(SNConfig);

type SNConfigCollection = NOCCollectionRWSync<SNConfig>;


#[derive(Debug)]
enum SyncSNResult {
    Success,
    ErrorState,
    Failed,
}

#[derive(Clone)]
pub struct SNConfigManager {
    name_resolver: NameResolver,
    meta_cache: MetaCacheRef,
    root_state: GlobalStateOutputProcessorRef,
    noc: NamedObjectCacheRef,

    sn_list: Arc<Mutex<Vec<(DeviceId, Device)>>>,
    coll: Arc<OnceCell<SNConfigCollection>>,
}

impl SNConfigManager {
    pub fn new(
        name_resolver: NameResolver,
        meta_cache: MetaCacheRef,
        root_state: GlobalStateOutputProcessorRef,
        noc: NamedObjectCacheRef,
    ) -> Self {
        Self {
            name_resolver,
            meta_cache,
            root_state,
            noc,
            sn_list: Arc::new(Mutex::new(vec![])),
            coll: Arc::new(OnceCell::new()),
        }
    }

    pub fn get_sn_list(&self) -> Vec<(DeviceId, Device)> {
        self.sn_list.lock().unwrap().clone()
    }

    pub async fn init(&self) -> BuckyResult<()> {
        let coll = self.load_state().await?;
        if let Err(_) = self.coll.set(coll) {
            unreachable!();
        }

        let sn_config;
        {
            let cache = self.coll.get().unwrap().coll().read().unwrap();
            sn_config = cache.clone();
        }

        let mut flush_at_once = false;
        if let Some(id) = &sn_config.sn {
            if let Err(_) = self.load_sn_from_noc(id).await {
                flush_at_once = true;
            }
        }

        if sn_config.source == SNConfigListSource::Meta {
            if flush_at_once {
                self.name_resolver.reset_name(CYFS_SN_NAME);
            }

            let this = self.clone();
            async_std::task::spawn(async move {
                this.sync().await;
            });
        }

        Ok(())
    }

    async fn sync(&self) {
        let mut next_interval = 60;
        loop {
            let ret = self.sync_once().await;
            let interval = match ret {
                SyncSNResult::Success => 60 * 60 * 24,
                SyncSNResult::ErrorState => 60 * 60,
                SyncSNResult::Failed => {
                    let ret = next_interval;
                    next_interval *= 2;
                    if next_interval > 60 * 60 {
                        next_interval = 60 * 60;
                    }

                    ret
                }
            };

            info!("sync sn config complete: result={:?}, will retry after {}s", ret, interval);
            async_std::task::sleep(std::time::Duration::from_secs(interval)).await;
        }
    }

    async fn sync_once(&self) -> SyncSNResult {
        let ret = self.name_resolver.lookup(CYFS_SN_NAME).await;
        match ret {
            Ok(NameResult::ObjectLink(id)) => {
                info!("got sn id from meta: {} -> {}", CYFS_SN_NAME, id);
                if let Err(e) = self.load_sn_from_meta(&id).await {
                    error!("got sn object from meta got error! id={}, {}", id, e);
                    SyncSNResult::ErrorState
                } else {
                    info!("got sn object from meta success! id={}", id);
                    SyncSNResult::Success
                }
            }
            Ok(NameResult::IPLink(value)) => {
                error!(
                    "got sn id from meta but not support! {} -> {}",
                    CYFS_SN_NAME, value
                );
                SyncSNResult::ErrorState
            }
            Err(e) if e.code() == BuckyErrorCode::NotFound => {
                error!("got sn id from meta but not found! {}", CYFS_SN_NAME);
                SyncSNResult::ErrorState
            }
            Err(e) => {
                error!("get sn from meta failed! name={}, {}", CYFS_SN_NAME, e);
                SyncSNResult::Failed
            }
        }
    }

    // load sn from noc on startup
    async fn load_sn_from_noc(&self, id: &ObjectId) -> BuckyResult<()> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: id.clone(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(object)) => {
                info!("load sn from noc: {}", id);
                let list = SNDirParser::parse(&id, &object.object.object_raw)?;
                *self.sn_list.lock().unwrap() = list;

                Ok(())
            }
            Ok(None) => {
                let msg = format!("load sn object from local noc not found! {}", id);
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            Err(e) => {
                error!("load sn from noc but failed! {}, {}", id, e);
                Err(e)
            }
        }
    }

    // load sn list in dir format from meta chain
    async fn load_sn_from_meta(&self, id: &ObjectId) -> BuckyResult<()> {
        let ret = self.meta_cache.get_object(id).await.map_err(|e| {
            error!("load sn from meta failed! id={}, {}", id, e);
            e
        })?;

        if ret.is_none() {
            let msg = format!("load sn from meta but not found! id={}", id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let object = ret.unwrap();
        let id = object.object.object_id();
        let list = SNDirParser::parse(&id, &object.object_raw)?;

        {
            let mut cache = self.coll.get().unwrap().coll().write().unwrap();
            if cache.source != SNConfigListSource::Meta {
                warn!(
                    "load sn from meta success but state's source is not meta! {:?}",
                    cache.source
                );
                return Ok(());
            }

            if cache.sn == Some(id) {
                warn!(
                    "load sn from meta success but state's id is the same! {:?}",
                    id
                );
                return Ok(());
            }

            info!(
                "update sn from mete and changed! sn={}, prev={:?}",
                id, cache.sn
            );
            cache.sn = Some(id.clone());
        }

        let coll = self.coll.get().unwrap().clone();
        async_std::task::spawn(async move {
            let _ = coll.save().await;
        });

        // save sn object to noc
        let mut object = NONObjectInfo::new(id, object.object_raw, None);
        object.decode()?;

        let put_req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        if let Err(e) = self.noc.put_object(&put_req).await {
            error!("save sn to noc failed! {}", e);
        }

        // update current sn list cache
        self.on_sn_list_changed(list);

        Ok(())
    }

    fn on_sn_list_changed(&self, list: Vec<(DeviceId, Device)>) {
        *self.sn_list.lock().unwrap() = list;
    }

    async fn load_state(&self) -> BuckyResult<SNConfigCollection> {
        let meta_path = format!("{}/sn", CYFS_GLOBAL_STATE_CONFIG_PATH);

        let data = NOCCollectionRWSync::<SNConfig>::new_global_state(
            self.root_state.clone(),
            Some(cyfs_core::get_system_dec_app().to_owned()),
            meta_path,
            None,
            "cyfs-sn-config",
            self.noc.clone(),
        );

        if let Err(e) = data.load().await {
            error!("load global state sn config failed! {}", e,);

            return Err(e);
        }

        info!(
            "load global state sn config! content={}",
            serde_json::to_string(&data.coll().read().unwrap() as &SNConfig).unwrap(),
        );

        Ok(data)
    }
}
