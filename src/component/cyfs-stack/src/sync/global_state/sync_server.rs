use super::super::protocol::*;
use super::sync_helper::*;
use crate::root_state_api::GlobalStateLocalService;
use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct GlobalStateSyncServer {
    state: GlobalStateSyncHelper,
}

impl GlobalStateSyncServer {
    pub fn new(
        state: GlobalStateLocalService,
        device_id: &DeviceId,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let state = GlobalStateSyncHelper::new(state, device_id, noc);

        Self { state }
    }

    pub async fn sync_diff(&self, req: &SyncDiffRequest) -> BuckyResult<SyncDiffResponse> {
        let ret = self.state.load_target(req).await?;
        if ret.is_none() {
            let resp = SyncDiffResponse {
                revision: 0,
                target: None,
                objects: vec![],
            };

            return Ok(resp);
        }

        let (target, revision) = ret.unwrap();
        match req.current {
            Some(current) => {
                if current == target {
                    info!(
                        "sync diff but target is the same! target={}, dec={:?}, path={}",
                        target, req.dec_id, req.path
                    );
                    let resp = SyncDiffResponse {
                        revision,
                        target: Some(target),
                        objects: vec![],
                    };

                    return Ok(resp);
                }

                // TODO calc diff， but allow some error， such as objectmap on leaf node
                let objects = self.try_load_object_map_to_list(&target, false).await?;
                let resp = SyncDiffResponse {
                    revision,
                    target: Some(target),
                    objects,
                };

                Ok(resp)
            }
            None => {
                let objects = self.try_load_object_map_to_list(&target, true).await?;
                let resp = SyncDiffResponse {
                    revision,
                    target: Some(target),
                    objects,
                };

                Ok(resp)
            }
        }
    }

    async fn try_load_object_map_to_list(
        &self,
        target: &ObjectId,
        load_subs: bool,
    ) -> BuckyResult<Vec<SelectResponseObjectInfo>> {
        let mut list = vec![];

        if target.obj_type_code() != ObjectTypeCode::ObjectMap {
            return Ok(list);
        }

        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(self.state.cache().clone());
        let ret = op_env_cache.get_object_map(&target).await?;
        if ret.is_none() {
            warn!(
                "sync diff load target from noc but not found! target={}",
                target
            );
            return Ok(list);
        }

        let obj = ret.unwrap();
        list.push(self.encode_object_map(target, &obj).await);

        // 如果一个objectmap在请求端不存在，并不代表里面所有的diff
        if load_subs {
            let mut sub_list = vec![];
            let o = obj.lock().await;
            match o.list_subs(&op_env_cache, &mut sub_list).await {
                Ok(_count) => {
                    for item in sub_list {
                        match op_env_cache.get_object_map(&item).await {
                            Ok(Some(obj)) => {
                                list.push(self.encode_object_map(&item, &obj).await);
                            }
                            Ok(None) => {
                                error!("sync diff load objectmap sub item but not found! target={}, sub_item={}", target, item);
                            }
                            Err(e) => {
                                error!("sync diff load objectmap sub item error! target={}, sub_item={}, {}", target, item, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "sync diff load objectmap sub item but list! target={}, {}",
                        target, e
                    );
                }
            }
        }

        Ok(list)
    }

    async fn encode_object_map(
        &self,
        object_id: &ObjectId,
        obj: &ObjectMapRef,
    ) -> SelectResponseObjectInfo {
        let o = obj.lock().await;
        let object_raw = o.to_vec().unwrap();
        let info = NONObjectInfo::new(object_id.to_owned(), object_raw, None);

        SelectResponseObjectInfo {
            meta: SelectResponseObjectMetaInfo {
                size: info.object_raw.len() as u32,
                insert_time: 0,
                create_dec_id: None,
                context: None,
                last_access_rpath: None,
                access_string: None,
            },
            object: Some(info),
        }
    }
}
