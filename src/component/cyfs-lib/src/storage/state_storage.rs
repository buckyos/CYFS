use crate::root_state::*;
use crate::UniCyfsStackRef;
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

struct StorageOpDataHolder {
    op_data: StorageOpData,
    keep_alive: Option<async_std::task::JoinHandle<()>>,
}

impl StorageOpDataHolder {
    fn start_keep_alive(&mut self) {
        let path_stub = self.op_data.path_stub.clone();
        let single_stub = self.op_data.single_stub.clone();
        let task = async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60 * 15)).await;

                if let Err(e) = path_stub.get_current_root().await {
                    error!("path-op-env stub keep alive but failed! {}", e);
                }
    
                if let Err(e) = single_stub.get_current_root().await {
                    error!("single-op-env stub keep alive but failed! {}", e);
                }
            }
        });

        assert!(self.keep_alive.is_none());
        self.keep_alive = Some(task);
    }

    async fn stop_keep_alive(&mut self, path: &str) {
        if let Some(task) = self.keep_alive.take() {
            info!("will stop state storage's op-env keep alive! path={}", path);
            task.cancel().await;
        } else {
            warn!("stop state storage's op-env keep alive task but not found! path={}", path);
        }
    }
}


pub struct StateStorage {
    path: String,
    content_type: ObjectMapSimpleContentType,
    global_state: GlobalStateOutputProcessorRef,
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    dirty: Arc<AtomicBool>,
    auto_save: Arc<AtomicBool>,

   
    op_data: OnceCell<StorageOpDataHolder>,
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
        global_state: GlobalStateOutputProcessorRef,
        path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> Self {
        Self {
            global_state,
            path: path.into(),
            content_type,
            target,
            dec_id,

            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
            op_data: OnceCell::new(),
        }
    }

    pub fn new_with_stack(
        stack: UniCyfsStackRef,
        category: GlobalStateCategory,
        path: impl Into<String>,
        content_type: ObjectMapSimpleContentType,
        target: Option<ObjectId>,
        dec_id: Option<ObjectId>,
    ) -> Self {
        let global_state = match category {
            GlobalStateCategory::RootState => stack.root_state().clone(),
            GlobalStateCategory::LocalCache => stack.local_cache().clone(),
        };

        Self {
            global_state,
            path: path.into(),
            content_type,
            target,
            dec_id,

            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
            op_data: OnceCell::new(),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn stub(&self) -> &SingleOpEnvStub {
        &self.op_data.get().unwrap().op_data.single_stub
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
        let op_data = self.op_data.get().unwrap().op_data.clone();

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

    async fn load(&self) -> BuckyResult<StorageOpDataHolder> {
        let dec_id = match &self.dec_id {
            Some(dec_id) => Some(dec_id.to_owned()),
            None => Some(cyfs_core::get_system_dec_app().to_owned()),
        };

        let stub = GlobalStateStub::new(self.global_state.clone(), self.target.clone(), dec_id);

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

        let mut holder = StorageOpDataHolder {
            op_data,
            keep_alive: None,
        };
        holder.start_keep_alive();

        Ok(holder)
    }

    // reload the target object and ignore all the unsaved changes!
    pub async fn reload(&self) -> BuckyResult<bool> {
        let op_data = &self.op_data.get().unwrap().op_data;

        let new = op_data.path_stub.get_by_path(&self.path).await?;

        let mut current = op_data.current.lock().await;
        if *current == new {
            return Ok(false);
        }

        match new {
            Some(ref obj) => {
                op_data.single_stub.load(obj.clone()).await?;
            }
            None => {
                op_data.single_stub.create_new(self.content_type).await?;
            }
        }

        *current = new;
        Ok(true)
    }

    pub async fn save(&self) -> BuckyResult<()> {
        if let Some(holder) = self.op_data.get() {
            Self::save_impl(&self.path, &self.dirty, &holder.op_data).await
        } else {
            Ok(())
        }
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

        if let Some(holder) = self.op_data.take() {
            self.abort_impl(holder).await;
        }
    }

    async fn abort_impl(&self, mut holder: StorageOpDataHolder) {
        info!("will abort state storage: path={}", self.path);

        // First should stop keep alive
        holder.stop_keep_alive(&self.path).await;

        let op_data = holder.op_data;

        // Before abord we should first hold the lock for update
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

    pub async fn next(&self, step: u32) -> BuckyResult<Vec<(String, ObjectId)>> {
        let list = self.storage.stub().next(step).await?;

        self.convert_list(list)
    }

    pub async fn reset(&self) -> BuckyResult<()> {
        self.storage.stub().reset().await
    }

    pub async fn list(&self) -> BuckyResult<Vec<(String, ObjectId)>> {
        let list = self.storage.stub().list().await?;

        self.convert_list(list)
    }

    fn convert_list(
        &self,
        list: Vec<ObjectMapContentItem>,
    ) -> BuckyResult<Vec<(String, ObjectId)>> {
        if list.is_empty() {
            return Ok(vec![]);
        }

        if list[0].content_type() != ObjectMapSimpleContentType::Map {
            let msg = format!(
                "state storage is not valid map type! path={}, type={}",
                self.storage().path,
                list[0].content_type().as_str()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let list = list
            .into_iter()
            .map(|item| match item {
                ObjectMapContentItem::Map(kp) => kp,
                _ => unreachable!(),
            })
            .collect();

        Ok(list)
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

    pub async fn next(&self, step: u32) -> BuckyResult<Vec<ObjectId>> {
        let list = self.storage.stub().next(step).await?;

        self.convert_list(list)
    }

    pub async fn reset(&self) -> BuckyResult<()> {
        self.storage.stub().reset().await
    }

    pub async fn list(&self) -> BuckyResult<Vec<ObjectId>> {
        let list = self.storage.stub().list().await?;

        self.convert_list(list)
    }

    fn convert_list(&self, list: Vec<ObjectMapContentItem>) -> BuckyResult<Vec<ObjectId>> {
        if list.is_empty() {
            return Ok(vec![]);
        }

        if list[0].content_type() != ObjectMapSimpleContentType::Set {
            let msg = format!(
                "state storage is not valid set type! path={}, type={}",
                self.storage().path,
                list[0].content_type().as_str()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let list = list
            .into_iter()
            .map(|item| match item {
                ObjectMapContentItem::Set(id) => id,
                _ => unreachable!(),
            })
            .collect();

        Ok(list)
    }
}


#[cfg(test)]
mod test {
    
    async fn test_keep_alive() {
        let task = async_std::task::spawn(async move {
            let mut index= 0 ;
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(1)).await;

                println!("keep alive: {}", index);
                index += 1;
            }
        });

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;
        println!("will cancel keep alive!");
        task.cancel().await;
        println!("end cancel keep alive!");

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;
    }

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            test_keep_alive().await;
        })
    }
}