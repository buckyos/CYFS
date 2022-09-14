use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait GlobalStateInputProcessor: Sync + Send + 'static {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef;

    fn get_category(&self) -> GlobalStateCategory;

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse>;

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse>;
}

pub type GlobalStateInputProcessorRef = Arc<Box<dyn GlobalStateInputProcessor>>;

#[async_trait::async_trait]
pub trait OpEnvInputProcessor: Sync + Send + 'static {
    fn get_category(&self) -> GlobalStateCategory;
    
    // single_op_env methods
    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()>;

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()>;

    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()>;

    // get_current_root
    async fn get_current_root(&self, req: OpEnvGetCurrentRootInputRequest) -> BuckyResult<OpEnvGetCurrentRootInputResponse>;

    // lock
    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()>;

    // transcation
    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse>;
    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()>;

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse>;
    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()>;
    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse>;

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse>;

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse>;
    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse>;
    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse>;

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse>;
    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()>;
    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse>;

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse>;
}

pub type OpEnvInputProcessorRef = Arc<Box<dyn OpEnvInputProcessor>>;

#[async_trait::async_trait]
pub trait GlobalStateAccessInputProcessor: Sync + Send + 'static {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse>;

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse>;
}

pub type GlobalStateAccessInputProcessorRef = Arc<Box<dyn GlobalStateAccessInputProcessor>>;