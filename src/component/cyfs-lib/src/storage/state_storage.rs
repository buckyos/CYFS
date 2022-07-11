use crate::root_state::*;
use crate::UniObjectStackRef;
use cyfs_base::*;

use async_std::sync::Mutex as AsyncMutex;
use once_cell::sync::OnceCell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Clone)]
struct StorageOpData {
    path_stub: PathOpEnvStub,
    single_stub: SingleOpEnvStub,
    current: Arc<AsyncMutex<Option<ObjectId>>>,
}

pub struct StateStorage {
    path: String,
    content_type: ObjectMapSimpleContentType,
    stack: UniObjectStackRef,
    category: GlobalStateCategory,
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    dirty: Arc<AtomicBool>,
    auto_save: Arc<AtomicBool>,
    op_data: OnceCell<StorageOpData>,
}

impl Drop for StateStorage {
    fn drop(&mut self) {
        async_std::task::block_on(async move {
            self.abort().await;
        })
    }
}

impl StateStorage {
    pub fn new(
        stack: UniObjectStackRef,
        category: GlobalStateCategory,
        path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> Self {
        Self {
            stack,
            category,
            path: path.into(),
            content_type,
            target,
            dec_id,

            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
            op_data: OnceCell::new(),
        }
    }

    pub fn stub(&self) -> &SingleOpEnvStub {
        &self.op_data.get().unwrap().single_stub
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.dirty.store(dirty, Ordering::SeqCst);
    }

    pub async fn init(&self) -> BuckyResult<()> {
        assert!(self.op_data.get().is_none());

        let op_data = self.load().await?;
        if let Err(_) = self.op_data.set(op_data) {
            unreachable!();
        }

        Ok(())
    }

    pub fn start_save(&self, dur: std::time::Duration) {
        use async_std::prelude::*;

        let ret = self
            .auto_save
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire);
        if ret.is_err() {
            warn!("storage already in saving state! path={}", self.path);
            return;
        }

        let auto_save = self.auto_save.clone();
        let path = self.path.clone();
        let dirty = self.dirty.clone();
        let op_data = self.op_data.get().unwrap().clone();

        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(dur);
            while let Some(_) = interval.next().await {
                if !auto_save.load(Ordering::SeqCst) {
                    warn!("storage auto save stopped! path={}", path);
                    break;
                }

                let _ = Self::save_impl(&path, &dirty, &op_data).await;
            }
        });
    }

    pub fn stop_save(&self) {
        if let Ok(_) =
            self.auto_save
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        {
            info!("stop state storage auto save! path={}", self.path);
        }
    }

    async fn load(&self) -> BuckyResult<StorageOpData> {
        let state = match self.category {
            GlobalStateCategory::RootState => self.stack.root_state().clone(),
            GlobalStateCategory::LocalCache => self.stack.local_cache().clone(),
        };

        let dec_id = match &self.dec_id {
            Some(dec_id) => Some(dec_id.to_owned()),
            None => Some(cyfs_core::get_system_dec_app().object_id().to_owned()),
        };

        let stub = GlobalStateStub::new(state, self.target.clone(), dec_id);

        let path_stub = stub.create_path_op_env().await?;
        path_stub
            .lock(vec![self.path.clone()], u64::MAX)
            .await
            .unwrap();

        let single_stub = stub.create_single_op_env().await?;

        let current = path_stub.get_by_path(&self.path).await?;
        match current {
            Some(ref obj) => {
                single_stub.load(obj.clone()).await?;
            }
            None => {
                single_stub.create_new(self.content_type).await?;
            }
        }

        let op_data = StorageOpData {
            path_stub,
            single_stub,
            current: Arc::new(AsyncMutex::new(current)),
        };

        Ok(op_data)
    }

    pub async fn save(&self) -> BuckyResult<()> {
        let op_data = self.op_data.get().unwrap();
        Self::save_impl(&self.path, &self.dirty, op_data).await
    }

    async fn save_impl(
        path: &str,
        dirty: &Arc<AtomicBool>,
        op_data: &StorageOpData,
    ) -> BuckyResult<()> {
        let ret = dirty.compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire);
        if ret.is_err() {
            return Ok(());
        }

        let ret = Self::commit_impl(path, op_data).await;

        if ret.is_err() {
            dirty.store(true, Ordering::SeqCst);
        }

        ret
    }

    pub async fn abort(&mut self) {
        self.stop_save();

        if let Some(op_data) = self.op_data.take() {
            self.abort_impl(op_data).await;
        }
    }

    async fn abort_impl(&self, op_data: StorageOpData) {
        info!("will abort state storage: path={}", self.path);
        
        // first hold the lock for update
        let mut _current = op_data.current.lock().await;

        if let Err(e) = op_data.single_stub.abort().await {
            error!(
                "abort state storage single stub error! path={}, {}",
                self.path, e
            );
        }

        if let Err(e) = op_data.path_stub.abort().await {
            error!(
                "abort state storage path stub error! path={}, {}",
                self.path, e
            );
        }

        self.set_dirty(false);
    }

    async fn commit_impl(path: &str, op_data: &StorageOpData) -> BuckyResult<()> {
        // first hold the lock for update
        let mut current = op_data.current.lock().await;

        let new = op_data.single_stub.update().await.map_err(|e| {
            error!("commit state storage failed! path={}, {}", path, e);
            e
        })?;

        if Some(new) == *current {
            debug!(
                "commit state storage but not changed! path={}, current={}",
                path, new
            );
            return Ok(());
        }

        match op_data
            .path_stub
            .set_with_path(path, &new, current.clone(), true)
            .await
        {
            Ok(_) => {
                info!(
                    "update state storage success! path={}, current={}, prev={:?}",
                    path, new, current
                );
            }
            Err(e) => {
                error!(
                    "update state storage but failed! path={}, current={}, prev={:?}, {}",
                    path, new, current, e
                );

                return Err(e);
            }
        }

        op_data.path_stub.update().await.map_err(|e| {
            error!(
                "commit state storage to global state failed! path={}, {}",
                path, e
            );
            e
        })?;

        *current = Some(new);

        info!(
            "commit state storage to global state success! path={}",
            path
        );

        Ok(())
    }
}

pub struct StateStorageMap {
    storage: StateStorage,
}

impl StateStorageMap {
    pub fn new(storage: StateStorage) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &StateStorage {
        &self.storage
    }

    pub fn into_storage(self) -> StateStorage {
        self.storage
    }

    pub async fn save(&self) -> BuckyResult<()> {
        self.storage.save().await
    }

    pub async fn abort(mut self) {
        self.storage.abort().await
    }

    pub async fn get(&self, key: impl Into<String>) -> BuckyResult<Option<ObjectId>> {
        self.storage.stub().get_by_key(key).await
    }

    pub async fn set(
        &self,
        key: impl Into<String>,
        value: &ObjectId,
    ) -> BuckyResult<Option<ObjectId>> {
        self.set_ex(key, value, None, true).await
    }

    pub async fn set_ex(
        &self,
        key: impl Into<String>,
        value: &ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let ret = self
            .storage
            .stub()
            .set_with_key(key, value, prev_value.clone(), auto_insert)
            .await?;

        if Some(*value) != ret {
            self.storage.set_dirty(true);
        }

        Ok(ret)
    }

    pub async fn insert(&self, key: impl Into<String>, value: &ObjectId) -> BuckyResult<()> {
        let ret = self.storage.stub().insert_with_key(key, value).await?;
        self.storage.set_dirty(true);

        Ok(ret)
    }

    pub async fn remove(&self, key: impl Into<String>) -> BuckyResult<Option<ObjectId>> {
        self.remove_ex(key, None).await
    }

    pub async fn remove_ex(
        &self,
        key: impl Into<String>,
        prev_value: Option<ObjectId>,
    ) -> BuckyResult<Option<ObjectId>> {
        let ret = self.storage.stub().remove_with_key(key, prev_value).await?;

        if ret.is_some() {
            self.storage.set_dirty(true);
        }

        Ok(ret)
    }
}

pub struct StateStorageSet {
    storage: StateStorage,
}

impl StateStorageSet {
    pub fn new(storage: StateStorage) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &StateStorage {
        &self.storage
    }

    pub fn into_storage(self) -> StateStorage {
        self.storage
    }

    pub async fn save(&self) -> BuckyResult<()> {
        self.storage.save().await
    }

    pub async fn abort(mut self) {
        self.storage.abort().await
    }

    pub async fn contains(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        self.storage.stub().contains(object_id).await
    }

    pub async fn insert(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let ret = self.storage.stub().insert(object_id).await?;
        if ret {
            self.storage.set_dirty(true);
        }

        Ok(ret)
    }

    pub async fn remove(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        let ret = self.storage.stub().remove(object_id).await?;
        if ret {
            self.storage.set_dirty(true);
        }

        Ok(ret)
    }
}
