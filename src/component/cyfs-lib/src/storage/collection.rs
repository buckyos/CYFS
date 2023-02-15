use super::storage::*;
use crate::prelude::NamedObjectCacheRef;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_debug::Mutex;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/*
基于storage的编码兼容处理
一般有三种编码格式：
1. 使用jsoncodec手工编解码，对于增加的字段，自己手动处理
2. 使用serde_json自动编解码，对于新增加的字段，要使用Option选项，否则会导致出现missing field导致无法解码
3. 使用raw_codec自动编解码，不支持增删字段后的编解码，需要小心，改变结构定义后，需要处理解码失败导致load失败的情况
*/
pub trait CollectionCodec<T> {
    fn encode(&self) -> BuckyResult<Vec<u8>>;
    fn decode(buf: &[u8]) -> BuckyResult<T>;
}

impl<T> CollectionCodec<T> for T
where
    T: for<'de> RawDecode<'de> + RawEncode,
{
    fn encode(&self) -> BuckyResult<Vec<u8>> {
        self.to_vec()
    }

    fn decode(buf: &[u8]) -> BuckyResult<T> {
        T::clone_from_slice(&buf)
    }
}

#[macro_export]
macro_rules! declare_collection_codec_for_serde {
    ($T:ty) => {
        impl CollectionCodec<$T> for $T {
            fn encode(&self) -> cyfs_base::BuckyResult<Vec<u8>> {
                let body = serde_json::to_string(&self).map_err(|e| {
                    let msg = format!("encode to json error! {}", e);
                    log::error!("{}", msg);
                    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCode::InvalidFormat, msg)
                })?;
                Ok(body.into_bytes())
            }
            fn decode(buf: &[u8]) -> cyfs_base::BuckyResult<$T> {
                serde_json::from_slice(buf).map_err(|e| {
                    let msg = format!("decode from json error! {}", e);
                    log::error!("{}", msg);
                    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCode::InvalidFormat, msg)
                })
            }
        }
    };
}

#[macro_export]
macro_rules! declare_collection_codec_for_json_codec {
    ($T:ty) => {
        impl CollectionCodec<$T> for $T {
            fn encode(&self) -> cyfs_base::BuckyResult<Vec<u8>> {
                Ok(self.encode_string().into())
            }
            fn decode(buf: &[u8]) -> cyfs_base::BuckyResult<$T> {
                use std::str;
                let str_value = str::from_utf8(buf).map_err(|e| {
                    let msg = format!("not valid utf8 string format: {}", e);
                    log::error!("{}", msg);
                    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCode::InvalidFormat, msg)
                })?;
                Self::decode_string(str_value)
            }
        }
    };
}

pub struct NOCStorageWrapper {
    storage: Box<dyn NOCStorage>,
}

impl NOCStorageWrapper {
    pub fn new(id: &str, noc: NamedObjectCacheRef) -> Self {
        Self {
            storage: Box::new(NOCRawStorage::new(id, noc)),
        }
    }

    pub fn new_global_state(
        global_state: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        path: String,
        target: Option<ObjectId>,
        id: &str,
        noc: NamedObjectCacheRef,
    ) -> Self {
        Self {
            storage: Box::new(NOCGlobalStateStorage::new(
                global_state, dec_id, path, target, id, noc,
            )),
        }
    }

    pub async fn exists(id: &str, noc: &NamedObjectCacheRef) -> BuckyResult<bool> {
        NOCRawStorage::exists(id, noc).await
    }

    pub fn id(&self) -> &str {
        self.storage.id()
    }

    pub async fn load<T>(&self) -> BuckyResult<Option<T>>
    where
        T: CollectionCodec<T>,
    {
        match self.storage.load().await? {
            Some(buf) => {
                let coll = T::decode(&buf).map_err(|e| {
                    error!(
                        "decode storage buf to collection failed! id={}, {}",
                        self.id(),
                        e
                    );
                    e
                })?;

                Ok(Some(coll))
            }
            None => Ok(None),
        }
    }

    pub async fn save<T>(&self, data: &T) -> BuckyResult<()>
    where
        T: CollectionCodec<T>,
    {
        let buf = data.encode().map_err(|e| {
            error!(
                "convert collection to buf failed! id={}, {}",
                self.storage.id(),
                e
            );
            e
        })?;

        self.storage.save(buf).await
    }

    pub async fn delete(&self) -> BuckyResult<()> {
        self.storage.delete().await
    }
}

pub struct NOCCollection<T>
where
    T: Default + CollectionCodec<T>,
{
    coll: T,
    storage: NOCStorageWrapper,
    dirty: bool,
}

impl<T> NOCCollection<T>
where
    T: Default + CollectionCodec<T>,
{
    pub fn new(id: &str, noc: NamedObjectCacheRef) -> Self {
        Self {
            coll: T::default(),
            storage: NOCStorageWrapper::new(id, noc),
            dirty: false,
        }
    }

    pub fn id(&self) -> &str {
        self.storage.id()
    }

    pub fn coll(&self) -> &T {
        &self.coll
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn swap(&mut self, mut value: T) -> T {
        std::mem::swap(&mut self.coll, &mut value);
        value
    }

    pub async fn exists(id: &str, noc: &NamedObjectCacheRef) -> BuckyResult<bool> {
        NOCStorageWrapper::exists(id, noc).await
    }

    pub async fn load(&mut self) -> BuckyResult<()> {
        match self.storage.load().await? {
            Some(coll) => {
                self.coll = coll;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn save(&mut self) -> BuckyResult<()> {
        if self.is_dirty() {
            self.set_dirty(false);

            self.storage.save(&self.coll).await.map_err(|e| {
                self.set_dirty(true);
                e
            })
        } else {
            Ok(())
        }
    }

    pub async fn delete(&mut self) -> BuckyResult<()> {
        self.storage.delete().await?;

        // FIXME 删除后是否要置空?
        // self.coll = T::default();

        Ok(())
    }
}

use std::ops::Deref;
use std::ops::DerefMut;

pub trait NOCCollectionWithLock<T>
where
    T: Default + ?Sized + Send + 'static,
{
    fn read(&self) -> Box<dyn Deref<Target = T> + '_>;
    fn write(&self) -> Box<dyn DerefMut<Target = T> + '_>;
    //fn replace(&self, value: T);
}

struct NOCCollectionWithMutex<T>
where
    T: Default + ?Sized + Send + 'static,
{
    coll: Mutex<T>,
}

impl<T> NOCCollectionWithMutex<T>
where
    T: Default + ?Sized + Send + 'static,
{
    fn new() -> Self {
        Self {
            coll: Mutex::new(T::default()),
        }
    }
}

impl<T> NOCCollectionWithLock<T> for NOCCollectionWithMutex<T>
where
    T: Default + ?Sized + Send + 'static,
{
    fn read(&self) -> Box<dyn Deref<Target = T> + '_> {
        Box::new(self.coll.lock().unwrap())
    }
    fn write(&self) -> Box<dyn DerefMut<Target = T> + '_> {
        Box::new(self.coll.lock().unwrap())
    }
}

use std::sync::RwLock;

struct NOCCollectionWithRWLock<T>
where
    T: Default + ?Sized + Send + 'static,
{
    coll: RwLock<T>,
}

impl<T> NOCCollectionWithRWLock<T>
where
    T: Default + ?Sized + Send + 'static,
{
    fn new() -> Self {
        Self {
            coll: RwLock::new(T::default()),
        }
    }
}

impl<T> NOCCollectionWithLock<T> for NOCCollectionWithRWLock<T>
where
    T: Default + ?Sized + Send + 'static,
{
    fn read(&self) -> Box<dyn Deref<Target = T> + '_> {
        Box::new(self.coll.read().unwrap())
    }
    fn write(&self) -> Box<dyn DerefMut<Target = T> + '_> {
        Box::new(self.coll.write().unwrap())
    }
}

pub struct NOCCollectionSync<T>
where
    T: Default + CollectionCodec<T> + Send + 'static,
{
    coll: Arc<Mutex<T>>,
    storage: Arc<Box<dyn NOCStorage>>,

    dirty: Arc<AtomicBool>,
    auto_save: Arc<AtomicBool>,
}

impl<T> Clone for NOCCollectionSync<T>
where
    T: Default + CollectionCodec<T> + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            coll: self.coll.clone(),
            storage: self.storage.clone(),
            dirty: self.dirty.clone(),
            auto_save: self.auto_save.clone(),
        }
    }
}

impl<T> NOCCollectionSync<T>
where
    T: Default + CollectionCodec<T> + Send + 'static,
{
    pub fn new(id: &str, noc: NamedObjectCacheRef) -> Self {
        let storage = NOCRawStorage::new(id, noc);

        Self {
            coll: Arc::new(Mutex::new(T::default())),
            storage: Arc::new(Box::new(storage)),
            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn new_global_state(
        global_state: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        path: String,
        target: Option<ObjectId>,
        id: &str,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let storage = NOCGlobalStateStorage::new(
            global_state, dec_id, path, target, id, noc,
        );

        Self {
            coll: Arc::new(Mutex::new(T::default())),
            storage: Arc::new(Box::new(storage)),
            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    pub fn set_dirty(&self, dirty: bool) -> bool {
        self.dirty.swap(dirty, Ordering::SeqCst)
    }

    pub fn coll(&self) -> &Arc<Mutex<T>> {
        &self.coll
    }

    pub fn id(&self) -> &str {
        self.storage.id()
    }

    pub fn swap(&mut self, mut value: T) -> T {
        {
            let mut cur = self.coll.lock().unwrap();
            std::mem::swap(&mut *cur, &mut value);
        }

        self.set_dirty(true);
        value
    }

    pub async fn load(&self) -> BuckyResult<()> {
        match self.storage.load().await? {
            Some(buf) => {
                let coll = T::decode(&buf).map_err(|e| {
                    error!(
                        "decode storage buf to collection failed! id={}, {}",
                        self.id(),
                        e
                    );
                    e
                })?;

                *self.coll.lock().unwrap() = coll;
                Ok(())
            }
            None => Ok(()),
        }
    }

    // 保存，必须正确的调用set_dirty才会发起真正的保存操作
    pub async fn save(&self) -> BuckyResult<()> {
        if self.set_dirty(false) {
            self.save_impl().await.map_err(|e| {
                self.set_dirty(true);
                e
            })
        } else {
            Ok(())
        }
    }

    // 异步的保存，必须正确的调用set_dirty才会发起真正的保存操作
    pub fn async_save(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            let _r = this.save().await;
        });
    }

    async fn save_impl(&self) -> BuckyResult<()> {
        let buf = {
            let coll = self.coll.lock().unwrap();
            coll.encode().map_err(|e| {
                error!(
                    "convert collection to buf failed! id={}, {}",
                    self.storage.id(),
                    e
                );
                e
            })?
        };

        self.storage.save(buf).await
    }

    pub async fn delete(&self) -> BuckyResult<()> {
        self.storage.delete().await?;

        // 删除后需要停止自动保存
        self.stop_save();

        // FIXME 删除后是否要置空?
        // self.coll = T::default();

        Ok(())
    }

    // 开始定时保存操作
    pub fn start_save(&self, dur: std::time::Duration) {
        use async_std::prelude::*;

        let ret = self
            .auto_save
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire);
        if ret.is_err() {
            warn!("storage already in saving state! id={}", self.id());
            return;
        }

        let this = self.clone();
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(dur);
            while let Some(_) = interval.next().await {
                if !this.auto_save.load(Ordering::SeqCst) {
                    warn!("storage auto save stopped! id={}", this.id());
                    break;
                }
                let _ = this.save().await;
            }
        });
    }

    pub fn stop_save(&self) {
        if let Ok(_) =
            self.auto_save
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        {
            info!("will stop storage auto save! id={}", self.id());
        }
    }
}

pub struct NOCCollectionRWSync<T>
where
    T: Default + CollectionCodec<T> + Send + Sync + 'static,
{
    coll: Arc<RwLock<T>>,
    storage: Arc<Box<dyn NOCStorage>>,

    dirty: Arc<AtomicBool>,

    auto_save: Arc<AtomicBool>,
}


impl<T> Clone for NOCCollectionRWSync<T>
where
    T: Default + CollectionCodec<T> + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self {
            coll: self.coll.clone(),
            storage: self.storage.clone(),
            dirty: self.dirty.clone(),
            auto_save: self.auto_save.clone(),
        }
    }
}

impl<T> NOCCollectionRWSync<T>
where
    T: Default + CollectionCodec<T> + Send + Sync + 'static,
{
    pub fn new(id: &str, noc: NamedObjectCacheRef) -> Self {
        let noc = NOCRawStorage::new(id, noc);
        Self {
            coll: Arc::new(RwLock::new(T::default())),
            storage: Arc::new(Box::new(noc)),
            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn new_global_state(
        global_state: GlobalStateOutputProcessorRef,
        dec_id: Option<ObjectId>,
        path: String,
        target: Option<ObjectId>,
        id: &str,
        noc: NamedObjectCacheRef,
    ) -> Self {
        let storage = NOCGlobalStateStorage::new(
            global_state, dec_id, path, target, id, noc,
        );

        Self {
            coll: Arc::new(RwLock::new(T::default())),
            storage: Arc::new(Box::new(storage)),
            dirty: Arc::new(AtomicBool::new(false)),
            auto_save: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::SeqCst)
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.dirty.store(dirty, Ordering::SeqCst);
    }

    pub fn coll(&self) -> &Arc<RwLock<T>> {
        &self.coll
    }

    pub fn id(&self) -> &str {
        self.storage.id()
    }

    pub fn swap(&self, mut value: T) -> T {
        {
            let mut cur = self.coll.write().unwrap();
            std::mem::swap(&mut *cur, &mut value);
        }

        self.set_dirty(true);

        value
    }

    pub async fn load(&self) -> BuckyResult<()> {
        match self.storage.load().await? {
            Some(buf) => {
                let coll = T::decode(&buf).map_err(|e| {
                    error!(
                        "decode storage buf to collection failed! id={}, {}",
                        self.id(),
                        e
                    );
                    e
                })?;

                *self.coll.write().unwrap() = coll;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub async fn save(&self) -> BuckyResult<()> {
        if self.is_dirty() {
            self.set_dirty(false);

            self.save_impl().await.map_err(|e| {
                self.set_dirty(true);
                e
            })
        } else {
            Ok(())
        }
    }

    pub async fn save_impl(&self) -> BuckyResult<()> {
        let buf = {
            let coll = self.coll.read().unwrap();
            coll.encode().map_err(|e| {
                error!(
                    "convert collection to buf failed! id={}, {}",
                    self.storage.id(),
                    e
                );
                e
            })?
        };

        self.storage.save(buf).await
    }

    pub async fn delete(&mut self) -> BuckyResult<()> {
        self.storage.delete().await?;

        // 删除后需要停止自动保存
        self.stop_save();

        // FIXME 删除后是否要置空?
        // self.coll = T::default();

        Ok(())
    }

    // 开始定时保存操作
    pub fn start_save(&self, dur: std::time::Duration) {
        use async_std::prelude::*;

        let ret = self
            .auto_save
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire);
        if ret.is_err() {
            warn!("storage already in saving state! id={}", self.id());
            return;
        }

        let this = self.clone();
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(dur);
            while let Some(_) = interval.next().await {
                if !this.auto_save.load(Ordering::SeqCst) {
                    warn!("storage auto save stopped! id={}", this.id());
                    break;
                }

                let _ = this.save().await;
            }
        });
    }

    pub fn stop_save(&self) {
        if let Ok(_) =
            self.auto_save
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        {
            info!("will stop storage auto save! id={}", self.id());
        }
    }
}
