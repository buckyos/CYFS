use super::output_request::*;
use cyfs_base::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait GlobalStateOutputProcessor: Sync + Send + 'static {
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

    // single_op_env methods
    async fn load(&self, req: OpEnvLoadOutputRequest) -> BuckyResult<()>;

    async fn load_by_path(&self, req: OpEnvLoadByPathOutputRequest) -> BuckyResult<()>;

    async fn create_new(&self, req: OpEnvCreateNewOutputRequest) -> BuckyResult<()>;

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
}

pub type OpEnvOutputProcessorRef = Arc<Box<dyn OpEnvOutputProcessor>>;


#[async_trait::async_trait]
pub trait GlobalStateAccessOutputProcessor: Sync + Send + 'static {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathOutputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathOutputResponse>;

    async fn list(
        &self,
        req: RootStateAccessListOutputRequest,
    ) -> BuckyResult<RootStateAccessListOutputResponse>;
}

pub type GlobalStateAccessOutputProcessorRef = Arc<Box<dyn GlobalStateAccessOutputProcessor>>;