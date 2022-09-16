use super::super::protocol::*;
use crate::root_state_api::*;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone)]
pub(crate) struct GlobalStateSyncHelper {
    state: GlobalStateLocalService,
    device_id: DeviceId,
    noc: NamedObjectCacheRef,
    noc_cache: ObjectMapNOCCacheRef,
    cache: ObjectMapRootCacheRef,
}

impl GlobalStateSyncHelper {
    pub fn new(
        state: GlobalStateLocalService,
        device_id: &DeviceId,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let noc_cache = ObjectMapNOCCacheAdapter::new_noc_cache(&device_id, noc.clone());
        let cache = ObjectMapRootMemoryCache::new_default_ref(noc_cache.clone());

        Self {
            state,
            device_id: device_id.to_owned(),
            noc,
            noc_cache,
            cache,
        }
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub fn global_state(&self) -> &GlobalStateLocalService {
        &self.state
    }

    pub fn noc(&self) -> &NamedObjectCacheRef {
        &self.noc
    }

    pub fn cache(&self) -> &ObjectMapRootCacheRef {
        &self.cache
    }

    pub fn new_op_env_cache(&self) -> ObjectMapOpEnvCacheRef {
        ObjectMapOpEnvMemoryCache::new_ref(self.cache.clone())
    }

    pub async fn load_target(&self, req: &SyncDiffRequest) -> BuckyResult<Option<(ObjectId, u64)>> {
        let ret = self.load_root(req).await?;
        if ret.is_none() {
            warn!("sync diff but root not found! dec={:?}", req.dec_id);
            return Ok(None);
        }

        let (root, revision) = ret.unwrap();
        let op_env_cache = self.new_op_env_cache();
        let path = ObjectMapPath::new(root, op_env_cache);
        let ret = path.get_by_path(&req.path).await;
        if ret.is_err() {
            warn!(
                "sync diff get_by_path error! dec={:?}, path={}, {}",
                req.dec_id,
                req.path,
                ret.unwrap_err(),
            );
            return Ok(None);
        }

        match ret.unwrap() {
            Some(target) => {
                info!(
                    "sync diff get_by_path got: target={}, dec={:?}, path={}",
                    target, req.dec_id, req.path
                );
                Ok(Some((target, revision)))
            }
            None => {
                warn!(
                    "sync diff but path not found! dec={:?}, path={}",
                    req.dec_id, req.path
                );
                Ok(None)
            }
        }
    }

    async fn load_root(&self, req: &SyncDiffRequest) -> BuckyResult<Option<(ObjectId, u64)>> {
        assert_eq!(req.category, GlobalStateCategory::RootState);

        match &req.dec_id {
            Some(dec_id) => {
                let ret = self.state.state().get_dec_root(dec_id).await?;
                if ret.is_none() {
                    return Ok(None);
                }

                let (_, revision, dec_root) = ret.unwrap();
                Ok(Some((dec_root, revision)))
            }
            None => {
                let ret = self.state.state().get_current_root();
                Ok(Some(ret))
            }
        }
    }
}
