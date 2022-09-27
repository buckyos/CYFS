use crate::root_state::*;
use crate::UniObjectStackRef;
use cyfs_base::*;

use std::sync::{Arc, RwLock};

#[derive(Clone)]
struct StorageOpData {
    single_stub: Option<Arc<SingleOpEnvStub>>,
    current: Option<ObjectId>,
}

pub struct StateView {
    path: String,
    content_type: ObjectMapSimpleContentType,
    global_state: GlobalStateOutputProcessorRef,
    target: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    op_data: RwLock<StorageOpData>,
}

impl Drop for StateView {
    fn drop(&mut self) {
        async_std::task::block_on(async move {
            self.abort().await;
        })
    }
}

impl StateView {
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

            op_data: RwLock::new(StorageOpData {
                single_stub: None,
                current: None,
            }),
        }
    }

    pub fn new_with_stack(
        stack: UniObjectStackRef,
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

            op_data: RwLock::new(StorageOpData {
                single_stub: None,
                current: None,
            }),
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn stub(&self) -> Option<Arc<SingleOpEnvStub>> {
        self.op_data.read().unwrap().single_stub.clone()
    }

    pub async fn abort(&mut self) {
        let op_data = {
            let mut data = self.op_data.write().unwrap();
            let mut empty = StorageOpData {
                single_stub: None,
                current: None,
            };

            std::mem::swap(&mut *data, &mut empty);
            empty
        };

        if let Some(stub) = op_data.single_stub {
            match Arc::try_unwrap(stub) {
                Ok(stub) => {
                    if let Err(e) = stub.abort().await {
                        error!("abort storage view error! path={}, {}", self.path, e);
                    }
                }
                Err(_) => {
                    error!(
                        "abort storage view but single_op_env ref count > 1! path={}",
                        self.path
                    );
                }
            }
        }
    }

    pub async fn load(&self) -> BuckyResult<bool> {
        let target = self.load_target().await?;
        {
            let current = self.op_data.read().unwrap();
            if current.current == target {
                return Ok(false);
            }

            info!(
                "will load state view: path={}, {:?} -> {:?}",
                self.path, current.current, target
            );
        }

        let single_stub = match &target {
            Some(id) => {
                let stub = self.create_stub();
                let stub = stub.create_single_op_env().await?;
                stub.load(id.to_owned()).await?;
                Some(Arc::new(stub))
            }
            None => None,
        };

        let op_data = StorageOpData {
            single_stub,
            current: target,
        };

        {
            let mut current = self.op_data.write().unwrap();
            *current = op_data;
        }

        Ok(true)
    }

    fn create_stub(&self) -> GlobalStateStub {
        let dec_id = match &self.dec_id {
            Some(dec_id) => Some(dec_id.to_owned()),
            None => Some(cyfs_core::get_system_dec_app().to_owned()),
        };

        let stub = GlobalStateStub::new(self.global_state.clone(), self.target.clone(), dec_id);
        stub
    }

    async fn load_target(&self) -> BuckyResult<Option<ObjectId>> {
        let stub = self.create_stub();

        let path_stub = stub.create_path_op_env().await?;

        let current = path_stub.get_by_path(&self.path).await?;
        if let Err(e) = path_stub.abort().await {
            error!(
                "abort storage view path stub error! path={}, {}",
                self.path, e
            );
        }

        Ok(current)
    }
}

pub struct StateMapView {
    storage: StateView,
}

impl StateMapView {
    pub fn new(storage: StateView) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &StateView {
        &self.storage
    }

    pub fn into_storage(self) -> StateView {
        self.storage
    }

    pub async fn abort(mut self) {
        self.storage.abort().await
    }

    pub async fn get(&self, key: impl Into<String>) -> BuckyResult<Option<ObjectId>> {
        match self.storage.stub() {
            Some(stub) => stub.get_by_key(key).await,
            None => Ok(None),
        }
    }

    pub async fn next(&self, step: u32) -> BuckyResult<Vec<(String, ObjectId)>> {
        match self.storage.stub() {
            Some(stub) => {
                let list = stub.next(step).await?;

                self.convert_list(list)
            }
            None => Ok(vec![]),
        }
    }

    pub async fn reset(&self) -> BuckyResult<()> {
        match self.storage.stub() {
            Some(stub) => stub.reset().await,
            None => Ok(()),
        }
    }

    pub async fn list(&self) -> BuckyResult<Vec<(String, ObjectId)>> {
        match self.storage.stub() {
            Some(stub) => {
                let list = stub.list().await?;

                self.convert_list(list)
            }
            None => Ok(vec![]),
        }
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

pub struct StateSetView {
    storage: StateView,
}

impl StateSetView {
    pub fn new(storage: StateView) -> Self {
        Self { storage }
    }

    pub fn storage(&self) -> &StateView {
        &self.storage
    }

    pub fn into_storage(self) -> StateView {
        self.storage
    }

    pub async fn abort(mut self) {
        self.storage.abort().await
    }

    pub async fn contains(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        match self.storage.stub() {
            Some(stub) => stub.contains(object_id).await,
            None => Ok(false),
        }
    }

    pub async fn next(&self, step: u32) -> BuckyResult<Vec<ObjectId>> {
        match self.storage.stub() {
            Some(stub) => {
                let list = stub.next(step).await?;

                self.convert_list(list)
            }
            None => Ok(vec![]),
        }
    }

    pub async fn reset(&self) -> BuckyResult<()> {
        match self.storage.stub() {
            Some(stub) => stub.reset().await,
            None => Ok(()),
        }
    }

    pub async fn list(&self) -> BuckyResult<Vec<ObjectId>> {
        match self.storage.stub() {
            Some(stub) => {
                let list = stub.list().await?;

                self.convert_list(list)
            }
            None => Ok(vec![]),
        }
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

use std::collections::{HashMap, HashSet};

pub struct StateMapViewCache {
    cache: RwLock<HashMap<String, ObjectId>>,
    view: StateMapView,
}

impl StateMapViewCache {
    pub fn new(storage: StateView) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            view: StateMapView::new(storage),
        }
    }

    pub async fn load(&self) -> BuckyResult<bool> {
        let changed = self.view.storage().load().await?;
        if !changed {
            return Ok(false);
        }

        let list = self.view.list().await?;
        let cache = list.into_iter().collect();

        {
            let mut current = self.cache.write().unwrap();
            *current = cache;
        }

        Ok(true)
    }

    pub fn get(&self, key: &str) -> Option<ObjectId> {
        self.cache.read().unwrap().get(key).cloned()
    }

    pub fn coll(&self) -> &RwLock<HashMap<String, ObjectId>> {
        &self.cache
    }
}

pub struct StateSetViewCache {
    cache: RwLock<HashSet<ObjectId>>,
    view: StateSetView,
}

impl StateSetViewCache {
    pub fn new(storage: StateView) -> Self {
        Self {
            cache: RwLock::new(HashSet::new()),
            view: StateSetView::new(storage),
        }
    }

    pub async fn reload(&self) -> BuckyResult<bool> {
        let changed = self.view.storage().load().await?;
        if !changed {
            return Ok(false);
        }

        let list = self.view.list().await?;
        let cache = list.into_iter().collect();

        {
            let mut current = self.cache.write().unwrap();
            *current = cache;
        }

        Ok(true)
    }

    pub fn contains(&self, object_id: &ObjectId) -> bool {
        self.cache.read().unwrap().contains(object_id)
    }

    pub fn coll(&self) -> &RwLock<HashSet<ObjectId>> {
        &self.cache
    }
}
