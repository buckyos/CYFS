use super::output_request::*;
use super::processor::*;
use cyfs_base::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DecRootInfo {
    pub root: ObjectId,
    pub revision: u64,
    pub dec_root: ObjectId,
}

#[derive(Clone)]
pub struct GlobalStateStub {
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,
    processor: GlobalStateOutputProcessorRef,
}

impl GlobalStateStub {
    pub fn new(processor: GlobalStateOutputProcessorRef, target: Option<ObjectId>, dec_id: Option<ObjectId>) -> Self {
        Self { processor, target, dec_id }
    }

    // return (global_root, revision,)
    pub async fn get_current_root(&self) -> BuckyResult<(ObjectId, u64)> {
        let mut req = RootStateGetCurrentRootOutputRequest::new_global();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_current_root(req).await?;
        Ok((resp.root, resp.revision))
    }

    // return (global_root, revision, dec_root)
    pub async fn get_dec_root(&self) -> BuckyResult<DecRootInfo> {
        let mut req = RootStateGetCurrentRootOutputRequest::new_dec();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_current_root(req).await?;

        let info = DecRootInfo {
            root: resp.root,
            revision: resp.revision,
            dec_root: resp.dec_root.unwrap(),
        };

        Ok(info)
    }

    pub async fn create_path_op_env(&self) -> BuckyResult<PathOpEnvStub> {
        let mut req = RootStateCreateOpEnvOutputRequest::new(ObjectMapOpEnvType::Path);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.create_op_env(req).await?;
        let op_env = PathOpEnvStub::new(resp, self.target.clone(), self.dec_id.clone());
        Ok(op_env)
    }

    pub async fn create_single_op_env(&self) -> BuckyResult<SingleOpEnvStub> {
        let mut req = RootStateCreateOpEnvOutputRequest::new(ObjectMapOpEnvType::Single);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.create_op_env(req).await?;
        let op_env = SingleOpEnvStub::new(resp, self.target.clone(), self.dec_id.clone());
        Ok(op_env)
    }
}

#[derive(Clone)]
pub struct SingleOpEnvStub {
    processor: OpEnvOutputProcessorRef,
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,
}

impl SingleOpEnvStub {
    pub(crate) fn new(processor: OpEnvOutputProcessorRef, target: Option<ObjectId>, dec_id: Option<ObjectId>) -> Self {
        Self { processor, target, dec_id }
    }

    // init methods
    pub async fn create_new(&self, content_type: ObjectMapSimpleContentType) -> BuckyResult<()> {
        let mut req = OpEnvCreateNewOutputRequest::new(content_type);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.create_new(req).await
    }
    pub async fn load(&self, target: ObjectId) -> BuckyResult<()> {
        let mut req = OpEnvLoadOutputRequest::new(target);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.load(req).await
    }
    pub async fn load_by_path(&self, path: impl Into<String>) -> BuckyResult<()> {
        let mut req = OpEnvLoadByPathOutputRequest::new(path.into());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.load_by_path(req).await
    }

    // get_current_root
    pub async fn get_current_root(&self) -> BuckyResult<ObjectId> {
        let mut req = OpEnvGetCurrentRootOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_current_root(req).await?;
        Ok(resp.dec_root)
    }

    // map methods
    pub async fn get_by_key(&self, key: impl Into<String>) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvGetByKeyOutputRequest::new_key(key);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_by_key(req).await?;
        Ok(resp.value)
    }

    pub async fn insert_with_key(
        &self,
        key: impl Into<String>,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        let mut req = OpEnvInsertWithKeyOutputRequest::new_key_value(key, value.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.insert_with_key(req).await?;
        Ok(())
    }

    pub async fn set_with_key(
        &self,
        key: impl Into<String>,
        value: &ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvSetWithKeyOutputRequest::new_key_value(
            key,
            value.to_owned(),
            prev_value,
            auto_insert,
        );
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.set_with_key(req).await?;
        Ok(resp.prev_value)
    }

    pub async fn remove_with_key(
        &self,
        key: impl Into<String>,
        prev_value: Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvRemoveWithKeyOutputRequest::new_key(key, prev_value);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.remove_with_key(req).await?;
        Ok(resp.value)
    }

    // set methods
    pub async fn contains(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut req = OpEnvContainsOutputRequest::new(object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.contains(req).await?;
        Ok(resp.result)
    }

    pub async fn insert(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut req = OpEnvInsertOutputRequest::new(object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.insert(req).await?;
        Ok(resp.result)
    }

    pub async fn remove(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut req = OpEnvRemoveOutputRequest::new(object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.remove(req).await?;
        Ok(resp.result)
    }

    // transcation
    pub async fn update(&self) -> BuckyResult<ObjectId> {
        let mut req = OpEnvCommitOutputRequest::new_update();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.commit(req).await?;
        Ok(resp.dec_root)
    }

    pub async fn commit(self) -> BuckyResult<ObjectId> {
        let mut req = OpEnvCommitOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.commit(req).await?;
        Ok(resp.dec_root)
    }

    pub async fn abort(self) -> BuckyResult<()> {
        let mut req = OpEnvAbortOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.abort(req).await?;
        Ok(())
    }

    // iterator
    pub async fn next(&self, step: u32) -> BuckyResult<Vec<ObjectMapContentItem>> {
        let mut req = OpEnvNextOutputRequest::new(step);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.next(req).await?;
        Ok(resp.list)
    }

    pub async fn reset(&self) -> BuckyResult<()> {
        let mut req = OpEnvResetOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.reset(req).await
    }

    // metadata
    async fn metadata(&self) -> BuckyResult<ObjectMapMetaData> {
        let mut req = OpEnvMetadataOutputRequest::new(None);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.metadata(req).await?;
        let metadata = ObjectMapMetaData {
            content_mode: resp.content_mode,
            content_type: resp.content_type,
            count: resp.count,
            size: resp.size,
            depth: resp.depth,
        };

        Ok(metadata)
    }
}

#[derive(Clone)]
pub struct PathOpEnvStub {
    processor: OpEnvOutputProcessorRef,
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,
}

impl PathOpEnvStub {
    pub(crate) fn new(processor: OpEnvOutputProcessorRef, target: Option<ObjectId>, dec_id: Option<ObjectId>) -> Self {
        Self { processor, target, dec_id }
    }

    // get_current_root
    pub async fn get_current_root(&self) -> BuckyResult<DecRootInfo> {
        let mut req = OpEnvGetCurrentRootOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_current_root(req).await?;

        let info = DecRootInfo {
            root: resp.root,
            revision: resp.revision,
            dec_root: resp.dec_root,
        };

        Ok(info)
    }

    // lock
    pub async fn lock(&self, path_list: Vec<String>, duration_in_millsecs: u64) -> BuckyResult<()> {
        let mut req = OpEnvLockOutputRequest::new(path_list, duration_in_millsecs);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.lock(req).await
    }

    pub async fn try_lock(&self, path_list: Vec<String>, duration_in_millsecs: u64) -> BuckyResult<()> {
        let mut req = OpEnvLockOutputRequest::new_try(path_list, duration_in_millsecs);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.lock(req).await
    }

    // map methods
    pub async fn get_by_key(
        &self,
        path: impl Into<String>,
        key: impl Into<String>,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvGetByKeyOutputRequest::new_path_and_key(path, key);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_by_key(req).await?;
        Ok(resp.value)
    }

    pub async fn create_new(
        &self,
        path: impl Into<String>,
        key: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        let mut req =
            OpEnvCreateNewOutputRequest::new_with_path_and_key(path, key, content_type);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.create_new(req).await?;
        Ok(())
    }

    pub async fn insert_with_key(
        &self,
        path: impl Into<String>,
        key: impl Into<String>,
        value: &ObjectId,
    ) -> BuckyResult<()> {
        let mut req =
            OpEnvInsertWithKeyOutputRequest::new_path_and_key_value(path, key, value.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.insert_with_key(req).await?;
        Ok(())
    }

    pub async fn set_with_key(
        &self,
        path: impl Into<String>,
        key: impl Into<String>,
        value: &ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvSetWithKeyOutputRequest::new_path_and_key_value(
            path,
            key,
            value.to_owned(),
            prev_value,
            auto_insert,
        );
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.set_with_key(req).await?;
        Ok(resp.prev_value)
    }

    pub async fn remove_with_key(
        &self,
        path: impl Into<String>,
        key: impl Into<String>,
        prev_value: Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvRemoveWithKeyOutputRequest::new_path_and_key(path, key, prev_value);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.remove_with_key(req).await?;
        Ok(resp.value)
    }

    // map methods with full_path
    pub async fn get_by_path(&self, full_path: impl Into<String>) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvGetByKeyOutputRequest::new_full_path(full_path);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.get_by_key(req).await?;
        Ok(resp.value)
    }

    pub async fn create_new_with_path(
        &self,
        full_path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
    ) -> BuckyResult<()> {
        let mut req =
            OpEnvCreateNewOutputRequest::new_with_full_path(full_path, content_type);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.create_new(req).await?;
        Ok(())
    }

    pub async fn insert_with_path(&self, full_path: &str, value: &ObjectId) -> BuckyResult<()> {
        let mut req =
            OpEnvInsertWithKeyOutputRequest::new_full_path_and_value(full_path, value.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.insert_with_key(req).await?;
        Ok(())
    }

    pub async fn set_with_path(
        &self,
        full_path: impl Into<String>,
        value: &ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvSetWithKeyOutputRequest::new_full_path_and_value(
            full_path,
            value.to_owned(),
            prev_value,
            auto_insert,
        );
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.set_with_key(req).await?;
        Ok(resp.prev_value)
    }

    pub async fn remove_with_path(
        &self,
        full_path: impl Into<String>,
        prev_value: Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let mut req = OpEnvRemoveWithKeyOutputRequest::new_full_path(full_path, prev_value);
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.remove_with_key(req).await?;
        Ok(resp.value)
    }

    // set methods
    pub async fn contains(
        &self,
        path: impl Into<String>,
        object_id: &ObjectId,
    ) -> BuckyResult<bool> {
        let mut req = OpEnvContainsOutputRequest::new_path(path, object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.contains(req).await?;
        Ok(resp.result)
    }

    pub async fn insert(&self, path: impl Into<String>, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut req = OpEnvInsertOutputRequest::new_path(path, object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.insert(req).await?;
        Ok(resp.result)
    }

    pub async fn remove(&self, path: impl Into<String>, object_id: &ObjectId) -> BuckyResult<bool> {
        let mut req = OpEnvRemoveOutputRequest::new_path(path, object_id.to_owned());
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.remove(req).await?;
        Ok(resp.result)
    }

    // transcation
    pub async fn update(&self) -> BuckyResult<DecRootInfo> {
        let mut req = OpEnvCommitOutputRequest::new_update();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.commit(req).await?;

        let info = DecRootInfo {
            root: resp.root,
            revision: resp.revision,
            dec_root: resp.dec_root,
        };

        Ok(info)
    }

    pub async fn commit(self) -> BuckyResult<DecRootInfo> {
        let mut req = OpEnvCommitOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.commit(req).await?;

        let info = DecRootInfo {
            root: resp.root,
            revision: resp.revision,
            dec_root: resp.dec_root,
        };

        Ok(info)
    }

    pub async fn abort(self) -> BuckyResult<()> {
        let mut req = OpEnvAbortOutputRequest::new();
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        self.processor.abort(req).await?;
        Ok(())
    }

    // metadata
    async fn metadata(&self, path: impl Into<String>) -> BuckyResult<ObjectMapMetaData> {
        let mut req = OpEnvMetadataOutputRequest::new(Some(path.into()));
        req.common.target = self.target.clone();
        req.common.dec_id = self.dec_id.clone();

        let resp = self.processor.metadata(req).await?;
        let metadata = ObjectMapMetaData {
            content_mode: resp.content_mode,
            content_type: resp.content_type,
            count: resp.count,
            size: resp.size,
            depth: resp.depth,
        };

        Ok(metadata)
    }
}
