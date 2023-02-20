use super::def::GlobalStateAccessMode;
use super::output_request::*;
use crate::GlobalStateCategory;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait GlobalStateOutputProcessor: Sync + Send + 'static {
    fn get_category(&self) -> GlobalStateCategory;

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootOutputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootOutputResponse>;

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvOutputRequest,
    ) -> BuckyResult<OpEnvOutputProcessorRef>;
}

pub type GlobalStateOutputProcessorRef = Arc<Box<dyn GlobalStateOutputProcessor>>;

#[async_trait::async_trait]
pub trait OpEnvOutputProcessor: Sync + Send + 'static {
    // 获取当前op_env的托管sid
    fn get_sid(&self) -> u64;

    fn get_category(&self) -> GlobalStateCategory;

    // single_op_env methods
    async fn load(&self, req: OpEnvLoadOutputRequest) -> BuckyResult<()>;

    async fn load_by_path(&self, req: OpEnvLoadByPathOutputRequest) -> BuckyResult<()>;

    async fn create_new(&self, req: OpEnvCreateNewOutputRequest) -> BuckyResult<()>;

    // get_current_root
    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootOutputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootOutputResponse>;

    // lock and transcation
    async fn lock(&self, req: OpEnvLockOutputRequest) -> BuckyResult<()>;
    async fn commit(&self, req: OpEnvCommitOutputRequest)
        -> BuckyResult<OpEnvCommitOutputResponse>;
    async fn abort(&self, req: OpEnvAbortOutputRequest) -> BuckyResult<()>;

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataOutputRequest,
    ) -> BuckyResult<OpEnvMetadataOutputResponse>;

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyOutputRequest,
    ) -> BuckyResult<OpEnvGetByKeyOutputResponse>;
    async fn insert_with_key(&self, req: OpEnvInsertWithKeyOutputRequest) -> BuckyResult<()>;
    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyOutputResponse>;

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyOutputResponse>;

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsOutputRequest,
    ) -> BuckyResult<OpEnvContainsOutputResponse>;
    async fn insert(&self, req: OpEnvInsertOutputRequest)
        -> BuckyResult<OpEnvInsertOutputResponse>;
    async fn remove(&self, req: OpEnvRemoveOutputRequest)
        -> BuckyResult<OpEnvRemoveOutputResponse>;

    // iterator methods
    async fn next(&self, req: OpEnvNextOutputRequest) -> BuckyResult<OpEnvNextOutputResponse>;
    async fn reset(&self, req: OpEnvResetOutputRequest) -> BuckyResult<()>;

    async fn list(&self, req: OpEnvListOutputRequest) -> BuckyResult<OpEnvListOutputResponse>;
}

pub type OpEnvOutputProcessorRef = Arc<Box<dyn OpEnvOutputProcessor>>;

#[async_trait::async_trait]
pub trait GlobalStateAccessorOutputProcessor: Sync + Send + 'static {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessorGetObjectByPathOutputRequest,
    ) -> BuckyResult<RootStateAccessorGetObjectByPathOutputResponse>;

    async fn list(
        &self,
        req: RootStateAccessorListOutputRequest,
    ) -> BuckyResult<RootStateAccessorListOutputResponse>;
}

pub type GlobalStateAccessorOutputProcessorRef = Arc<Box<dyn GlobalStateAccessorOutputProcessor>>;

#[derive(Clone, Debug)]
pub struct GlobalStateDecRootInfo {
    pub dec_id: ObjectId,
    pub dec_root: ObjectId,
}

#[derive(Clone, Debug)]
pub struct GlobalStateRootInfo {
    pub global_root: ObjectId,
    pub revision: u64,

    pub dec_list: Vec<GlobalStateDecRootInfo>,
}

#[async_trait::async_trait]
pub trait GlobalStateRawProcessor: Send + Sync {
    fn isolate_id(&self) -> &ObjectId;

    fn category(&self) -> GlobalStateCategory;

    fn access_mode(&self) -> GlobalStateAccessMode;

    // return (global_root, revision)
    fn get_current_root(&self) -> (ObjectId, u64);

    fn get_root_revision(&self, root: &ObjectId) -> Option<u64>;

    fn root_cache(&self) -> &ObjectMapRootCacheRef;

    fn is_dec_exists(&self, dec_id: &ObjectId) -> bool;

    async fn get_dec_root_info_list(&self) -> BuckyResult<GlobalStateRootInfo>;

    // return (global_root, revision, dec_root)
    async fn get_dec_root(
        &self,
        dec_id: &ObjectId,
    ) -> BuckyResult<Option<(ObjectId, u64, ObjectId)>>;

    async fn get_dec_root_manager(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<ObjectMapRootManagerRef>;
}

pub type GlobalStateRawProcessorRef = Arc<Box<dyn GlobalStateRawProcessor>>;

#[derive(Clone, Debug)]
pub struct GlobalStateIsolateInfo {
    pub isolate_id: ObjectId,
    pub owner: Option<ObjectId>,
    pub create_time: u64,
}

#[async_trait::async_trait]
pub trait GlobalStateManagerRawProcessor: Send + Sync {
    // get all isolates of specified category
    async fn get_isolate_list(&self, category: GlobalStateCategory) -> Vec<GlobalStateIsolateInfo>;

    // get relate methods
    async fn get_root_state(&self, isolate_id: &ObjectId) -> Option<GlobalStateRawProcessorRef> {
        self.get_global_state(GlobalStateCategory::RootState, isolate_id)
            .await
    }

    async fn get_local_cache(&self, isolate_id: &ObjectId) -> Option<GlobalStateRawProcessorRef> {
        self.get_global_state(GlobalStateCategory::LocalCache, isolate_id)
            .await
    }

    async fn get_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
    ) -> Option<GlobalStateRawProcessorRef>;

    // loa relate methods
    async fn load_root_state(
        &self,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRawProcessorRef>> {
        self.load_global_state(
            GlobalStateCategory::RootState,
            isolate_id,
            owner,
            auto_create,
        )
        .await
    }

    async fn load_local_cache(
        &self,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRawProcessorRef>> {
        self.load_global_state(
            GlobalStateCategory::LocalCache,
            isolate_id,
            owner,
            auto_create,
        )
        .await
    }

    async fn load_global_state(
        &self,
        category: GlobalStateCategory,
        isolate_id: &ObjectId,
        owner: Option<ObjectId>,
        auto_create: bool,
    ) -> BuckyResult<Option<GlobalStateRawProcessorRef>>;
}

pub type GlobalStateManagerRawProcessorRef = Arc<Box<dyn GlobalStateManagerRawProcessor>>;

/*
async fn usage(state_manager: GlobalStateManagerRawProcessorRef) -> BuckyResult<()> {
    let isolate_id = ObjectId::default();
    let owner = Some(PeopleId::default().object_id().clone());
    let group_state = state_manager.load_root_state(&isolate_id, owner, true).await?.unwrap();

    // get dec's root, create if not exists
    let dec_id = cyfs_core::DecAppId::default();
    let dec_group_state = group_state.get_dec_root_manager(dec_id.object_id(), true).await?;

    let op_env = dec_group_state.create_op_env(None).unwrap();
    // do something with op_env
    op_env.commit().await.unwrap();

    Ok(())
}
*/
