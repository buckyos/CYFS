use super::super::client::SyncClientRequestor;
use super::super::protocol::*;
use super::assoc::AssociationObjects;
use super::cache::SyncObjectsStateCache;
use super::data::{ChunksCollector, DataSync};
use super::walker::*;
use cyfs_base::*;
use cyfs_lib::*;

use futures::future::{AbortHandle, Abortable};
use std::sync::{Arc, Mutex};

// sync的重试间隔
const SYNC_RETRY_MIN_INTERVAL_SECS: u64 = 10;
const SYNC_RETRY_MAX_INTERVAL_SECS: u64 = 60 * 5;
const SYNC_RETRY_MAX_TIMES: u64 = 5;

pub(super) struct ObjectMapSync {
    target: ObjectId,
    cache: ObjectMapOpEnvCacheRef,
    state_cache: SyncObjectsStateCache,
    requestor: Arc<SyncClientRequestor>,
    noc: Box<dyn NamedObjectCache>,
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
        noc: Box<dyn NamedObjectCache>,
        device_id: DeviceId,
        data_sync: DataSync,
    ) -> Self {
        let chunks_collector = ChunksCollector::new(noc.clone_noc(), device_id.clone());

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
            if list.is_empty() && self.chunks_collector.is_empty() {
                info!("sync object & chunk list complete! target={}", self.target);
                break Ok(());
            }

            // sync objects
            if !list.is_empty() {
                // filter the missing objects
                let list = self.state_cache.filter_missing(list);
                if !list.is_empty() {
                    if let Err(e) = self.sync_objects_with_assoc(list, had_err).await {
                        error!(
                            "sync object list error! now will stop sync target={}",
                            self.target,
                        );
                        break Err(e);
                    }
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
    async fn sync_objects_with_assoc(
        &self,
        mut list: Vec<ObjectId>,
        had_err: &mut bool,
    ) -> BuckyResult<()> {
        loop {
            let mut assoc = AssociationObjects::new(self.chunks_collector.clone());

            let mut sync_list = vec![];
            {
                std::mem::swap(&mut list, &mut sync_list);
            }

            if let Err(e) = self.sync_objects(sync_list, had_err, &mut assoc).await {
                error!(
                    "sync object list error! now will stop sync target={}",
                    self.target,
                );
                break Err(e);
            }

            let assoc_list = assoc.into_list();
            if assoc_list.is_empty() {
                break Ok(());
            }

            // filter the missing objects
            let assoc_list = self.state_cache.filter_missing(assoc_list);
            if assoc_list.is_empty() {
                return Ok(());
            }

            // filter the already exists objects
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

            if list.is_empty() {
                break Ok(());
            }

            debug!("will sync assoc list: {:?}", list);
        }
    }

    async fn sync_objects(
        &self,
        list: Vec<ObjectId>,
        had_err: &mut bool,
        assoc_objects: &mut AssociationObjects,
    ) -> BuckyResult<()> {
        // 重试间隔
        let mut retry_interval = SYNC_RETRY_MIN_INTERVAL_SECS;
        let mut retry_times = 0;

        loop {
            match self
                .sync_objects_once(list.clone(), had_err, assoc_objects)
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
    ) -> BuckyResult<()> {
        debug!("will sync objects: {:?}", list);

        let sync_req = SyncObjectsRequest {
            begin_seq: 0,
            end_seq: u64::MAX,
            list: list.clone(),
        };

        let sync_resp = self.requestor.sync_objects(sync_req).await?;

        // try cache the missing object list
        list.into_iter().for_each(|id| {
            let exists = sync_resp
                .objects
                .iter()
                .find(|item| {
                    let object = item.object.as_ref().unwrap();
                    object.object_id == id
                })
                .is_some();
            if !exists {
                self.state_cache.miss_object(&id);
            }
        });

        // extract all assoc objects/chunks
        sync_resp.objects.iter().for_each(|item| {
            assoc_objects.append(item.object.as_ref().unwrap());
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
                    if let Err(_e) = self.put_object_map(object) {
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
                    if let Err(_e) = self.put_others(object).await {
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

    fn put_object_map(&self, object: NONObjectInfo) -> BuckyResult<()> {
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

        self.cache.put_object_map(&object.object_id, object_map)?;

        Ok(())
    }

    async fn put_others(&self, object: NONObjectInfo) -> BuckyResult<()> {
        let req = NamedObjectCacheInsertObjectRequest {
            protocol: NONProtocol::Native,
            source: self.device_id.clone(),
            object_id: object.object_id.clone(),
            dec_id: None,
            object: object.object.unwrap(),
            object_raw: object.object_raw,
            flags: 0,
        };

        match self.noc.insert_object(&req).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCacheInsertResult::Accept
                    | NamedObjectCacheInsertResult::Updated => {
                        info!(
                            "sync diff insert object to noc success: {}",
                            object.object_id
                        );
                    }
                    NamedObjectCacheInsertResult::AlreadyExists => {
                        warn!(
                            "sync diff insert object but already exists: {}",
                            object.object_id
                        );
                    }
                    NamedObjectCacheInsertResult::Merged => {
                        warn!(
                            "sync diff insert object but signs merged success: {}",
                            object.object_id
                        );
                    }
                }

                Ok(())
            }
            Err(e) => {
                error!(
                    "sync diff insert object to noc failed: {} {}",
                    object.object_id, e
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
