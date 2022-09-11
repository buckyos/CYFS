use crate::prelude::*;
use crate::root_state::*;
use crate::AccessString;
use crate::NONObjectInfo;
use crate::RequestSourceInfo;
use cyfs_base::*;
use cyfs_core::*;

use std::sync::atomic::{AtomicU64, Ordering};

struct NOCStorageRawHelper {
    id: String,
    noc: NamedObjectCacheRef,
    last_update_time: AtomicU64,
}

impl NOCStorageRawHelper {
    pub fn new(id: impl Into<String>, noc: NamedObjectCacheRef) -> Self {
        Self {
            id: id.into(),
            noc,
            last_update_time: AtomicU64::new(0),
        }
    }

    pub async fn load(&self, object_id: &ObjectId) -> BuckyResult<Option<Vec<u8>>> {
        let req = NamedObjectCacheGetObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            last_access_rpath: None,
        };

        let resp = self.noc.get_object(&req).await?;
        match resp {
            Some(data) => {
                match Storage::raw_decode(&data.object.object_raw) {
                    Ok((storage, _)) => {
                        // 缓存当前object的修改时间
                        let update_time = storage.body().as_ref().unwrap().update_time();
                        self.last_update_time.store(update_time, Ordering::Relaxed);

                        Ok(Some(storage.into_value()))
                    }
                    Err(e) => {
                        error!(
                            "decode storage object error: id={}, storage={}, {}",
                            self.id, object_id, e
                        );
                        Err(e)
                    }
                }
            }

            None => {
                info!(
                    "storage not found in noc: id={}, storage={}",
                    self.id, object_id,
                );
                Ok(None)
            }
        }
    }

    pub async fn save(&self, buf: Vec<u8>, with_hash: bool) -> BuckyResult<StorageId> {
        let mut storage: Storage = if with_hash {
            StorageObj::create_with_hash(&self.id, buf)
        } else {
            StorageObj::create(&self.id, buf)
        };

        // 检查一下body的更新时间，确保更新
        let old_update_time = self.last_update_time.load(Ordering::Relaxed);
        let mut now = storage.body().as_ref().unwrap().update_time();
        if now < old_update_time {
            warn!(
                "storage new time is older than current! now={}, cur={}",
                now, old_update_time
            );
            now = old_update_time + 1;
            storage.body_mut().as_mut().unwrap().set_update_time(now);
        }

        self.save_to_noc(storage).await
    }

    async fn save_to_noc(&self, storage: Storage) -> BuckyResult<StorageId> {
        let storage_id = storage.storage_id();
        info!(
            "now will save storage to noc: id={}, storage={}",
            self.id, storage_id
        );

        let object_raw = storage.to_vec().unwrap();
        let object = NONObjectInfo::new_from_object_raw(object_raw)?;

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            last_access_rpath: None,
            context: None,
            access_string: Some(AccessString::dec_default().value()),
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::Accept
                    | NamedObjectCachePutObjectResult::Updated => {
                        info!(
                            "insert storage to noc success! id={}, storage={}",
                            self.id, req.object.object_id
                        );
                        Ok(storage_id)
                    }
                    r @ _ => {
                        // 不应该到这里？因为修改后的update_time已经会被更新
                        // FIXME 如果触发了本地时间回滚之类的问题，这里是否需要强制delete然后再插入？
                        error!(
                            "update storage to noc but alreay exist! id={}, storage={}, result={:?}",
                            self.id, req.object.object_id, r
                        );

                        Err(BuckyError::from(BuckyErrorCode::AlreadyExists))
                    }
                }
            }
            Err(e) => {
                error!(
                    "insert storage to noc error! id={}, storage={}, {}",
                    self.id, req.object.object_id, e
                );
                Err(e)
            }
        }
    }

    // 从noc删除当前storage对象
    pub async fn delete(&self, object_id: &ObjectId) -> BuckyResult<()> {
        let req = NamedObjectCacheDeleteObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object_id: object_id.to_owned(),
            flags: 0,
        };

        let resp = self.noc.delete_object(&req).await?;
        if resp.deleted_count > 0 {
            info!(
                "delete storage object from noc successs: id={}, storage={}",
                self.id, req.object_id
            );
        } else {
            warn!(
                "delete storage object but not found: id={}, storage={}",
                self.id, req.object_id,
            );
        }

        Ok(())
    }
}

#[async_trait::async_trait]
pub trait NOCStorage: Send + Sync {
    fn id(&self) -> &str;
    async fn load(&self) -> BuckyResult<Option<Vec<u8>>>;
    async fn save(&self, buf: Vec<u8>) -> BuckyResult<()>;
    async fn delete(&self) -> BuckyResult<()>;
}

pub struct NOCGlobalStateStorage {
    global_state: GlobalStateOutputProcessorRef,
    dec_id: Option<ObjectId>,
    path: String,
    target: Option<ObjectId>,
    noc: NOCStorageRawHelper,
}

impl NOCGlobalStateStorage {
    pub fn new(
        global_state: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        path: String,
        target: Option<ObjectId>,
        id: &str,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let noc = NOCStorageRawHelper::new(id, noc);

        Self {
            global_state,
            dec_id,
            path,
            target,
            noc,
        }
    }

    fn create_global_stub(&self) -> GlobalStateStub {
        let dec_id = match &self.dec_id {
            Some(dec_id) => Some(dec_id.to_owned()),
            None => Some(cyfs_core::get_system_dec_app().object_id().to_owned()),
        };

        let stub = GlobalStateStub::new(self.global_state.clone(), self.target.clone(), dec_id);
        stub
    }
}

#[async_trait::async_trait]
impl NOCStorage for NOCGlobalStateStorage {
    fn id(&self) -> &str {
        &self.noc.id
    }

    async fn load(&self) -> BuckyResult<Option<Vec<u8>>> {
        let stub = self.create_global_stub();

        let path_stub = stub.create_path_op_env().await?;
        let current = path_stub.get_by_path(&self.path).await?;
        match current {
            Some(id) => {
                let ret = self.noc.load(&id).await.map_err(|mut e| {
                    let msg = format!(
                        "load storage from noc failed! id={}, stroage={}, path={}, dec={:?}, {}",
                        self.noc.id, id, self.path, self.dec_id, e,
                    );
                    error!("{}", msg);
                    e.set_msg(msg);
                    e
                })?;

                match ret {
                    Some(data) => Ok(Some(data)),
                    None => {
                        warn!("load storage from noc but not found! id={}, stroage={}, path={}, dec={:?}",
                        self.noc.id, id, self.path, self.dec_id);

                        Ok(None)
                    }
                }
            }
            None => {
                warn!(
                    "global state storage load from path but not found! id={}, path={}, dec={:?}",
                    self.noc.id, self.path, self.dec_id
                );
                Ok(None)
            }
        }
    }

    async fn save(&self, buf: Vec<u8>) -> BuckyResult<()> {
        // First save as storage object to noc
        let storage_id = self.noc.save(buf, true).await.map_err(|mut e| {
            let msg = format!(
                "save storage to noc failed! id={}, path={}, dec={:?}, {}",
                self.noc.id, self.path, self.dec_id, e,
            );
            error!("{}", msg);
            e.set_msg(msg);
            e
        })?;

        // Then update the global state to save the object_id
        let stub = self.create_global_stub();
        let path_stub = stub.create_path_op_env().await?;

        path_stub
            .set_with_path(&self.path, storage_id.object_id(), None, true)
            .await
            .map_err(|mut e| {
                let msg = format!(
                    "save storage to global state failed! id={}, path={}, dec={:?}, {}",
                    self.noc.id, self.path, self.dec_id, e,
                );
                error!("{}", msg);
                e.set_msg(msg);
                e
            })?;

        path_stub.commit().await.map_err(|mut e| {
            let msg = format!(
                "commit storage to global state failed! id={}, path={}, dec={:?}, {}",
                self.noc.id, self.path, self.dec_id, e,
            );
            error!("{}", msg);
            e.set_msg(msg);
            e
        })?;

        info!(
            "save storage to global state success! id={}, path={}, dec={:?}",
            self.noc.id, self.path, self.dec_id
        );

        Ok(())
    }

    async fn delete(&self) -> BuckyResult<()> {
        // First update the global state to save the object_id
        let stub = self.create_global_stub();
        let path_stub = stub.create_path_op_env().await?;

        let ret = path_stub
            .remove_with_path(&self.path, None)
            .await
            .map_err(|mut e| {
                let msg = format!(
                    "remove storage from global state failed! id={}, path={}, dec={:?}, {}",
                    self.noc.id, self.path, self.dec_id, e,
                );
                error!("{}", msg);
                e.set_msg(msg);
                e
            })?;

        path_stub.commit().await.map_err(|mut e| {
            let msg = format!(
                "commit storage to global state failed! id={}, path={}, dec={:?}, {}",
                self.noc.id, self.path, self.dec_id, e,
            );
            error!("{}", msg);
            e.set_msg(msg);
            e
        })?;

        match ret {
            Some(object_id) => {
                // Then delete object from noc
                if let Err(e) = self.noc.delete(&object_id).await {
                    error!("delete storage from noc but failed! id={}, path={}, dec={:?}, storage={}, {}", 
                    self.noc.id, self.path, self.dec_id, object_id, e);
                }
            }
            None => {
                info!(
                    "delete storage from global state but not found! id={}, path={}, dec={:?}",
                    self.noc.id, self.path, self.dec_id,
                );
            }
        }

        Ok(())
    }
}

pub struct NOCRawStorage {
    noc: NOCStorageRawHelper,
    storage_id: StorageId,
}

impl NOCRawStorage {
    pub fn new(id: &str, noc: NamedObjectCacheRef) -> Self {
        let storage: Storage = StorageObj::create(id, Vec::new());

        let noc = NOCStorageRawHelper::new(id, noc);

        Self {
            noc,
            storage_id: storage.storage_id(),
        }
    }
}

#[async_trait::async_trait]
impl NOCStorage for NOCRawStorage {
    fn id(&self) -> &str {
        &self.noc.id
    }

    async fn load(&self) -> BuckyResult<Option<Vec<u8>>> {
        self.noc.load(self.storage_id.object_id()).await
    }

    async fn save(&self, buf: Vec<u8>) -> BuckyResult<()> {
        match self.noc.save(buf, false).await {
            Ok(id) => {
                assert_eq!(id, self.storage_id);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    async fn delete(&self) -> BuckyResult<()> {
        self.noc.delete(self.storage_id.object_id()).await
    }
}
