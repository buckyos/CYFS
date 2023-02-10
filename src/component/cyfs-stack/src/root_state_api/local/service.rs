use super::super::core::*;
use super::accessor_service::GlobalStateAccessorService;
use crate::config::StackGlobalConfig;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateLocalService {
    global_state: Arc<GlobalStateManager>,

    // only valid for global_state category
    accessor_service: Arc<GlobalStateAccessorService>,
}

impl GlobalStateLocalService {
    pub async fn load(
        category: GlobalStateCategory,
        device_id: &DeviceId,
        owner: Option<ObjectId>,
        noc: NamedObjectCacheRef,
        config: StackGlobalConfig,
    ) -> BuckyResult<Self> {
        let global_state =
            GlobalStateManager::load(category, device_id, owner, noc.clone(), config).await?;
        let global_state = Arc::new(global_state);

        let accessor_service =
            GlobalStateAccessorService::new(device_id.to_owned(), global_state.clone(), noc);

        let ret = Self {
            global_state,
            accessor_service: Arc::new(accessor_service),
        };

        Ok(ret)
    }

    pub fn state(&self) -> &Arc<GlobalStateManager> {
        &self.global_state
    }

    pub fn clone_global_state_processor(&self) -> GlobalStateInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn clone_op_env_processor(&self) -> OpEnvInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn clone_accessor_processor(&self) -> GlobalStateAccessorInputProcessorRef {
        self.accessor_service.clone_processor()
    }

    pub fn get_target_dec_id(common: &RootStateInputRequestCommon) -> BuckyResult<&ObjectId> {
        match &common.target_dec_id {
            Some(dec_id) => Ok(dec_id),
            None => Ok(&common.source.dec),
        }
    }

    pub fn get_op_env_target_dec_id(common: &OpEnvInputRequestCommon) -> BuckyResult<&ObjectId> {
        match &common.target_dec_id {
            Some(dec_id) => Ok(dec_id),
            None => Ok(&common.source.dec),
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateInputProcessor for GlobalStateLocalService {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef {
        Self::clone_op_env_processor(&self)
    }

    fn get_category(&self) -> GlobalStateCategory {
        self.global_state.category()
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        let resp = match req.root_type {
            RootStateRootType::Global => {
                let (root, revision) = self.global_state.get_current_root();

                RootStateGetCurrentRootInputResponse {
                    root,
                    revision,
                    dec_root: None,
                }
            }

            RootStateRootType::Dec => {
                let dec_id = Self::get_target_dec_id(&req.common)?;

                let ret = self.global_state.get_dec_root(&dec_id).await?;
                if ret.is_none() {
                    let msg = format!("get_dec_root but not found! dec={}", dec_id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                let info = ret.unwrap();
                RootStateGetCurrentRootInputResponse {
                    root: info.0,
                    revision: info.1,
                    dec_root: Some(info.2),
                }
            }
        };

        Ok(resp)
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        let dec_id = Self::get_target_dec_id(&req.common)?;

        let dec_root_manager = self.global_state.get_dec_root_manager(dec_id, true).await?;
        drop(dec_id);

        let access = if let Some(access) = req.access {
            Some(OpEnvPathAccess::new(&access.path, access.access))
        } else {
            None
        };

        let sid = match req.op_env_type {
            ObjectMapOpEnvType::Path => {
                let env = dec_root_manager
                    .create_managed_op_env(access, Some(req.common.source.clone().into()))
                    .await?;
                info!(
                    "create_path_op_env success! source={}, sid={}",
                    req.common.source,
                    env.sid()
                );
                env.sid()
            }

            ObjectMapOpEnvType::Single => {
                let env = dec_root_manager
                    .create_managed_single_op_env(access, Some(req.common.source.clone().into()))?;
                info!(
                    "create_single_op_env success! dec_id={}, sid={}",
                    req.common.source,
                    env.sid()
                );
                env.sid()
            }
        };

        let resp = RootStateCreateOpEnvInputResponse { sid };
        Ok(resp)
    }
}

#[async_trait::async_trait]
impl OpEnvInputProcessor for GlobalStateLocalService {
    fn get_category(&self) -> GlobalStateCategory {
        self.global_state.category()
    }

    // single_op_env methods
    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;
        let op_env = dec_root_manager
            .managed_envs()
            .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;

        if req.inner_path.is_some() {
            op_env.load_with_inner_path(&req.target, req.inner_path).await
        } else {
            op_env.load(&req.target).await
        }
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;
        let op_env = dec_root_manager
            .managed_envs()
            .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;

        op_env.load_by_path(&req.path).await
    }

    // for single and path
    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let resp = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                if req.key.is_none() {
                    let msg = format!("create_new but empty key param: req={}", req);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                let key = req.key.as_ref().unwrap();
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;

                match req.path {
                    Some(path) => op_env.create_new(&path, &key, req.content_type).await?,
                    None => op_env.create_new_with_path(&key, req.content_type).await?,
                }
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;

                op_env.create_new(req.content_type).await?
            }
        };

        Ok(resp)
    }

    // lock
    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;
        let op_env = dec_root_manager
            .managed_envs()
            .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
        op_env
            .lock_path(req.path_list, req.duration_in_millsecs, req.try_lock)
            .await
    }

    // get_current_root
    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootInputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let dec_root = dec_root_manager
            .managed_envs()
            .get_current_root(req.common.sid, Some(&req.common.source.into()))
            .await?;

        let resp = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let (root, revision) = self.global_state.get_dec_relation_root_info(&dec_root);

                OpEnvCommitInputResponse {
                    root,
                    revision,
                    dec_root,
                }
            }
            ObjectMapOpEnvType::Single => OpEnvCommitInputResponse {
                root: dec_root.clone(),
                revision: 0,
                dec_root,
            },
        };

        Ok(resp)
    }

    // transcation
    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let dec_root = match req.op_type {
            Some(OpEnvCommitOpType::Update) => {
                dec_root_manager
                    .managed_envs()
                    .update(req.common.sid, Some(&req.common.source.into()))
                    .await?
            }
            _ => {
                dec_root_manager
                    .managed_envs()
                    .commit(req.common.sid, Some(&req.common.source.into()))
                    .await?
            }
        };

        let resp = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let (root, revision) = self.global_state.get_dec_relation_root_info(&dec_root);

                OpEnvCommitInputResponse {
                    root,
                    revision,
                    dec_root,
                }
            }
            ObjectMapOpEnvType::Single => OpEnvCommitInputResponse {
                root: dec_root.clone(),
                revision: 0,
                dec_root,
            },
        };

        Ok(resp)
    }

    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        dec_root_manager
            .managed_envs()
            .abort(req.common.sid, Some(&req.common.source.into()))
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let value = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                match req.path {
                    Some(path) => op_env.get_by_key(&path, &req.key).await?,
                    None => op_env.get_by_path(&req.key).await?,
                }
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.get_by_key(&req.key).await?
            }
        };

        let resp = OpEnvGetByKeyInputResponse { value };

        Ok(resp)
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let value = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                match req.path {
                    Some(path) => op_env.insert_with_key(&path, &req.key, &req.value).await?,
                    None => op_env.insert_with_path(&req.key, &req.value).await?,
                }
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.insert_with_key(&req.key, &req.value).await?
            }
        };

        Ok(value)
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let prev_value = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                match req.path {
                    Some(path) => {
                        op_env
                            .set_with_key(
                                &path,
                                &req.key,
                                &req.value,
                                &req.prev_value,
                                req.auto_insert,
                            )
                            .await?
                    }
                    None => {
                        op_env
                            .set_with_path(&req.key, &req.value, &req.prev_value, req.auto_insert)
                            .await?
                    }
                }
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env
                    .set_with_key(&req.key, &req.value, &req.prev_value, req.auto_insert)
                    .await?
            }
        };

        let resp = OpEnvSetWithKeyInputResponse { prev_value };

        Ok(resp)
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let value = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                match req.path {
                    Some(path) => {
                        op_env
                            .remove_with_key(&path, &req.key, &req.prev_value)
                            .await?
                    }
                    None => op_env.remove_with_path(&req.key, &req.prev_value).await?,
                }
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.remove_with_key(&req.key, &req.prev_value).await?
            }
        };

        let resp = OpEnvRemoveWithKeyInputResponse { value };

        Ok(resp)
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let result = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => match req.path {
                Some(path) => {
                    let op_env = dec_root_manager
                        .managed_envs()
                        .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                    op_env.contains(&path, &req.value).await?
                }
                None => {
                    let msg = format!(
                        "call contains on path_op_env but path param not found! req={}",
                        req
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            },
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.contains(&req.value).await?
            }
        };

        let resp = OpEnvContainsInputResponse { result };

        Ok(resp)
    }

    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let result = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => match req.path {
                Some(path) => {
                    let op_env = dec_root_manager
                        .managed_envs()
                        .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                    op_env.insert(&path, &req.value).await?
                }
                None => {
                    let msg = format!(
                        "call insert on path_op_env but path param not found! req={}",
                        req
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            },
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.insert(&req.value).await?
            }
        };

        let resp = OpEnvInsertInputResponse { result };

        Ok(resp)
    }

    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let result = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => match req.path {
                Some(path) => {
                    let op_env = dec_root_manager
                        .managed_envs()
                        .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                    op_env.remove(&path, &req.value).await?
                }
                None => {
                    let msg = format!(
                        "call contains on path_op_env but path param not found! req={}",
                        req
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            },
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.remove(&req.value).await?
            }
        };

        let resp = OpEnvRemoveInputResponse { result };

        Ok(resp)
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;
        let op_env = dec_root_manager
            .managed_envs()
            .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
        let list = op_env.next(req.step as usize).await?;
        let resp = OpEnvNextInputResponse { list: list.list };

        Ok(resp)
    }

    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;
        let op_env = dec_root_manager
            .managed_envs()
            .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
        op_env.reset().await;

        Ok(())
    }

    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;

        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let list = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                if req.path.is_none() {
                    let msg = format!(
                        "call list on path_op_env but path param not found! req={}",
                        req
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.list(req.path.as_ref().unwrap()).await
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.list().await
            }
        }?;

        let resp = OpEnvListInputResponse { list: list.list };

        Ok(resp)
    }

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        let dec_id = Self::get_op_env_target_dec_id(&req.common)?;
        let dec_root_manager = self
            .global_state
            .get_dec_root_manager(dec_id, false)
            .await?;

        let value = match OpEnvSessionIDHelper::get_type(req.common.sid)? {
            ObjectMapOpEnvType::Path => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_path_op_env(req.common.sid, Some(&req.common.source.into()))?;

                if req.path.is_none() {
                    let msg = format!("get metadata for path_op_env but path not specified!");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                }

                op_env.metadata(req.path.as_ref().unwrap()).await?
            }
            ObjectMapOpEnvType::Single => {
                let op_env = dec_root_manager
                    .managed_envs()
                    .get_single_op_env(req.common.sid, Some(&req.common.source.into()))?;
                op_env.metadata().await?
            }
        };

        let resp = OpEnvMetadataInputResponse {
            content_mode: value.content_mode,
            content_type: value.content_type,
            count: value.count,
            size: value.size,
            depth: value.depth,
        };

        Ok(resp)
    }
}
