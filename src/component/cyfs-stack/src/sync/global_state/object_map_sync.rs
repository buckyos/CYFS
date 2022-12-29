use super::super::client::SyncClientRequestor;
use super::super::protocol::*;
use super::assoc::AssociationObjects;
use super::cache::SyncObjectsStateCache;
use super::data::{ChunksCollector, DataSync};
use super::dir_sync::*;
use super::walker::*;
use cyfs_base::*;
use cyfs_lib::*;

use cyfs_debug::Mutex;
use futures::future::{AbortHandle, Abortable};
use std::sync::Arc;

// sync的重试间隔
const SYNC_RETRY_MIN_INTERVAL_SECS: u64 = 10;
const SYNC_RETRY_MAX_INTERVAL_SECS: u64 = 60 * 5;
const SYNC_RETRY_MAX_TIMES: u64 = 5;

pub(super) struct ObjectMapSync {
    target: ObjectId,
    cache: ObjectMapOpEnvCacheRef,
    state_cache: SyncObjectsStateCache,
    requestor: Arc<SyncClientRequestor>,
    noc: NamedObjectCacheRef,
    device_id: DeviceId,

    sync_waker: Mutex<Option<AbortHandle>>,
    chunks_collector: ChunksCollector,

    data_sync: DataSync,
}

impl ObjectMapSync {
    pub(super) fn new(
        target: ObjectId,
        cache: ObjectMapOpEnvCacheRef,
        state_cache: SyncObjectsStateCache,
        requestor: Arc<SyncClientRequestor>,
        noc: NamedObjectCacheRef,
        device_id: DeviceId,
        data_sync: DataSync,
    ) -> Self {
        let chunks_collector = ChunksCollector::new(noc.clone(), device_id.clone());

        Self {
            target,
            cache,
            state_cache,
            requestor,
            noc,
            device_id,
            sync_waker: Mutex::new(None),
            chunks_collector,
            data_sync,
        }
    }

    pub fn wakeup_sync(&self) {
        let waker = self.sync_waker.lock().unwrap().take();
        if let Some(waker) = waker {
            info!("now will wakeup object sync! target={}", self.target);
            waker.abort();
        }
    }

    pub async fn sync(&self, had_err: &mut bool) -> BuckyResult<()> {
        info!("will sync target: {}", self.target);

        let walker = ObjectMapWalker::new(
            self.cache.clone(),
            self.target.clone(),
            self.chunks_collector.clone(),
        );
        walker.clone().start();

        loop {
            let list = walker.next(64).await;
            if list.is_empty() {
                info!("sync object complete! target={}", self.target);
                break Ok(());
            }

            if let Err(e) = self.sync_objects_with_assoc(list, had_err).await {
                break Err(e);
            }
        }
    }

    // sync objects with all assoc objects
    async fn sync_objects_with_assoc(
        &self,
        mut list: Vec<ObjectId>,
        had_err: &mut bool,
    ) -> BuckyResult<()> {
        let mut dir_sync = self.data_sync.create_dir_sync();

        loop {
            if list.is_empty() && self.chunks_collector.is_empty() && dir_sync.is_empty() {
                info!(
                    "sync object & chunk list & dir list complete! target={}",
                    self.target
                );
                break Ok(());
            }

            // sync objects
            let sync_list = self.state_cache.filter_missing(list);
            match self
                .sync_objects_with_assoc_once(sync_list, &mut dir_sync, had_err)
                .await
            {
                Ok(assoc_objects) => {
                    list = assoc_objects;
                }
                Err(e) => {
                    error!(
                        "sync object list error! now will stop sync target={}",
                        self.target,
                    );
                    break Err(e);
                }
            }

            // sync chunks
            let chunk_list = self.chunks_collector.detach_chunks();
            if !chunk_list.is_empty() {
                if let Err(e) = self.sync_chunks(chunk_list).await {
                    error!(
                        "sync chunks list error! now will stop sync target={}",
                        self.target,
                    );
                    break Err(e);
                }
            }
        }
    }
    // sync objects with all assoc objects
    async fn sync_objects_with_assoc_once(
        &self,
        list: Vec<ObjectId>,
        dir_sync: &mut DirListSync,
        had_err: &mut bool,
    ) -> BuckyResult<Vec<ObjectId>> {
        let mut assoc = AssociationObjects::new(self.chunks_collector.clone());

        // first sync objects
        if !list.is_empty() {
            debug!("will sync assoc list: {:?}", list);

            if let Err(e) = self.sync_objects(list, had_err, &mut assoc, dir_sync).await {
                error!(
                    "sync object list error! now will stop sync target={}",
                    self.target,
                );
                return Err(e);
            }
        }

        // then sync dirs once
        // dir_sync will try to parse dir and extract all relative objects and chunks
        dir_sync.sync_once(&mut assoc).await;

        let assoc_list = assoc.into_list();
        if assoc_list.is_empty() {
            return Ok(vec![]);
        }

        // filter the missing objects
        let assoc_list = self.state_cache.filter_missing(assoc_list);
        if assoc_list.is_empty() {
            return Ok(vec![]);
        }

        // filter the already exists objects
        let mut list = vec![];
        for id in assoc_list {
            match self.cache.exists(&id).await {
                Ok(exists) if !exists => {
                    list.push(id);
                }
                _ => {
                    // TODO wht should do if call exists error?
                }
            }
        }

        Ok(list)
    }

    async fn sync_objects(
        &self,
        list: Vec<ObjectId>,
        had_err: &mut bool,
        assoc_objects: &mut AssociationObjects,
        dir_sync: &mut DirListSync,
    ) -> BuckyResult<()> {
        // 重试间隔
        let mut retry_interval = SYNC_RETRY_MIN_INTERVAL_SECS;
        let mut retry_times = 0;

        loop {
            match self
                .sync_objects_once(list.clone(), had_err, assoc_objects, dir_sync)
                .await
            {
                Ok(_) => break,
                Err(e) => {
                    error!(
                        "sync objects error, target={}, now will retry after {} secs: {}",
                        self.target, retry_interval, e
                    );

                    // 等待重试，并允许被提前唤醒
                    let (abort_handle, abort_registration) = AbortHandle::new_pair();
                    let fut = Abortable::new(
                        async_std::task::sleep(std::time::Duration::from_secs(retry_interval)),
                        abort_registration,
                    );

                    {
                        *self.sync_waker.lock().unwrap() = Some(abort_handle);
                    }

                    match fut.await {
                        Ok(_) => {
                            info!("sync retry timeout: target={}", self.target);
                            let _ = self.sync_waker.lock().unwrap().take();
                        }
                        Err(futures::future::Aborted { .. }) => {
                            info!("sync retry wakeup: target={}", self.target);
                        }
                    };

                    retry_times += 1;
                    if retry_times > 5 {
                        error!(
                            "sync object extend max retry, now will stop, target={}",
                            self.target
                        );
                        return Err(e);
                    }

                    retry_interval *= 2;
                    if retry_interval >= SYNC_RETRY_MAX_INTERVAL_SECS {
                        retry_interval = SYNC_RETRY_MAX_INTERVAL_SECS;
                    }
                }
            }
        }

        Ok(())
    }

    async fn sync_objects_once(
        &self,
        list: Vec<ObjectId>,
        had_err: &mut bool,
        assoc_objects: &mut AssociationObjects,
        dir_sync: &mut DirListSync,
    ) -> BuckyResult<()> {
        debug!("will sync objects: {:?}", list);

        let sync_req = SyncObjectsRequest {
            begin_seq: 0,
            end_seq: u64::MAX,
            list: list.clone(),
        };

        let sync_resp = self.requestor.sync_objects(sync_req).await?;

        // try cache the missing object list
        for id in &list {
            let exists = sync_resp
                .objects
                .iter()
                .find(|item| {
                    let object = item.object.as_ref().unwrap();
                    object.object_id == *id
                })
                .is_some();
            if !exists {
                if *id == self.target {
                    let msg = format!("sync objects but target object is missing! {}", id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
                }

                self.state_cache.miss_object(&id);
            }
        }

        // extract all assoc objects/chunks
        sync_resp.objects.iter().for_each(|item| {
            let info = item.object.as_ref().unwrap();
            assoc_objects.append(info);

            if info.object_id.obj_type_code() == ObjectTypeCode::Dir {
                dir_sync.append_dir(info);
            }
        });

        self.save_objects(sync_resp.objects, had_err).await;

        Ok(())
    }

    pub async fn save_objects(&self, list: Vec<SelectResponseObjectInfo>, had_err: &mut bool) {
        for item in list {
            let object = item.object.unwrap();
            let type_code = object.object_id.obj_type_code();
            match type_code {
                ObjectTypeCode::ObjectMap => {
                    if let Err(_e) = self.put_object_map(
                        object,
                        item.meta.access_string.map(|v| AccessString::new(v)),
                    ) {
                        *had_err = true;
                    }
                }
                _ => {
                    // deal with assoc chunks
                    match type_code {
                        ObjectTypeCode::File | ObjectTypeCode::Dir => {
                            self.chunks_collector
                                .append_object(&object.object_id, &object.object.as_ref().unwrap());
                        }
                        _ => {}
                    }

                    // save objects to noc
                    if let Err(_e) = self.put_others(item.meta, object).await {
                        *had_err = true;
                    }
                }
            }
        }

        // need save the pending objectmaps to noc, on safety
        if let Err(e) = self.cache.commit().await {
            error!("sync diff flush objectmap to noc error! {}", e);
            *had_err = true;
        }
    }

    fn put_object_map(
        &self,
        object: NONObjectInfo,
        access: Option<AccessString>,
    ) -> BuckyResult<()> {
        let obj = object.object.unwrap();
        let obj = Arc::try_unwrap(obj).unwrap();
        let object_map = match obj {
            AnyNamedObject::Standard(v) => match v {
                StandardObject::ObjectMap(v) => v,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        let id = object_map.flush_id();
        if id != object.object_id {
            let msg = format!(
                "sync object but flush id got unmatch result! id={}, flush_id={}",
                object.object_id, id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        self.cache
            .put_object_map(&object.object_id, object_map, access)?;

        Ok(())
    }

    async fn put_others(
        &self,
        meta: SelectResponseObjectMetaInfo,
        object: NONObjectInfo,
    ) -> BuckyResult<()> {
        let source = RequestSourceInfo::new_local_dec(meta.create_dec_id);
        let req = NamedObjectCachePutObjectRequest {
            source,
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: meta.context,
            last_access_rpath: meta.last_access_rpath,
            access_string: meta.access_string,
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::Accept
                    | NamedObjectCachePutObjectResult::Updated => {
                        info!(
                            "sync diff insert object to noc success: {}",
                            req.object.object_id
                        );
                    }
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        warn!(
                            "sync diff insert object but already exists: {}",
                            req.object.object_id
                        );
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        warn!(
                            "sync diff insert object but signs merged success: {}",
                            req.object.object_id
                        );
                    }
                }

                Ok(())
            }
            Err(e) => {
                error!(
                    "sync diff insert object to noc failed: {} {}",
                    req.object.object_id, e
                );
                Err(e)
            }
        }
    }

    async fn sync_chunks(&self, list: Vec<ChunkId>) -> BuckyResult<()> {
        for sub_list in list.chunks(1024) {
            self.sync_chunks_impl(sub_list).await?;
        }

        Ok(())
    }

    async fn sync_chunks_impl(&self, list: &[ChunkId]) -> BuckyResult<()> {
        // 重试间隔
        let mut retry_interval = SYNC_RETRY_MIN_INTERVAL_SECS;
        let mut retry_times = 0;

        loop {
            match self.data_sync.sync_chunks(list.to_owned()).await {
                Ok(_) => break,
                Err(e) => {
                    error!(
                        "sync chunks error, target={}, now will retry after {} secs: {}",
                        self.target, retry_interval, e
                    );

                    // 等待重试，并允许被提前唤醒
                    let (abort_handle, abort_registration) = AbortHandle::new_pair();
                    let fut = Abortable::new(
                        async_std::task::sleep(std::time::Duration::from_secs(retry_interval)),
                        abort_registration,
                    );

                    {
                        *self.sync_waker.lock().unwrap() = Some(abort_handle);
                    }

                    match fut.await {
                        Ok(_) => {
                            info!("sync chunks retry timeout: target={}", self.target);
                            let _ = self.sync_waker.lock().unwrap().take();
                        }
                        Err(futures::future::Aborted { .. }) => {
                            info!("sync chunks retry wakeup: target={}", self.target);
                        }
                    };

                    retry_times += 1;
                    if retry_times > 5 {
                        error!(
                            "sync chunks extend max retry, now will stop, target={}",
                            self.target
                        );
                        return Err(e);
                    }

                    retry_interval *= 2;
                    if retry_interval >= SYNC_RETRY_MAX_INTERVAL_SECS {
                        retry_interval = SYNC_RETRY_MAX_INTERVAL_SECS;
                    }
                }
            }
        }

        Ok(())
    }
}
