use crate::meta::MetaCache;
use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use async_std::sync::Mutex as AsyncMutex;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use std::future::Future;
use std::sync::Arc;

const OBJECT_SEARCH_FLAG_NOC: u32 = 0x01;
const OBJECT_SEARCH_FLAG_META: u32 = 0x02;
const OBJECT_SEARCH_FLAG_ZONE: u32 = 0x04;

pub struct ObjectSearcherFlags;

impl ObjectSearcherFlags {
    pub fn full() -> u32 {
        OBJECT_SEARCH_FLAG_NOC | OBJECT_SEARCH_FLAG_META | OBJECT_SEARCH_FLAG_ZONE
    }

    pub fn none_local() -> u32 {
        OBJECT_SEARCH_FLAG_META | OBJECT_SEARCH_FLAG_ZONE
    }

    pub fn local_and_meta() -> u32 {
        OBJECT_SEARCH_FLAG_META | OBJECT_SEARCH_FLAG_NOC
    }
}

#[async_trait]
pub trait ObjectSearcher: Send + Sync + 'static {
    async fn search(
        &self,
        owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo>;

    async fn search_ex(
        &self,
        owner_id: Option<ObjectId>,
        object_id: &ObjectId,
        _flags: u32,
    ) -> BuckyResult<NONObjectInfo> {
        self.search(owner_id, object_id).await
    }
}

pub type ObjectSearcherRef = Arc<Box<dyn ObjectSearcher>>;

#[async_trait]
impl<F, Fut> ObjectSearcher for F
where
    F: Send + Sync + 'static + Fn(Option<ObjectId>, &ObjectId) -> Fut,
    Fut: Future<Output = BuckyResult<NONObjectInfo>> + Send + 'static,
{
    async fn search(
        &self,
        owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo> {
        let fut = (self)(owner_id, object_id);
        fut.await
    }
}

pub(crate) struct MetaSearcher {
    meta_cache: Box<dyn MetaCache>,
}

impl MetaSearcher {
    pub fn new(meta_cache: Box<dyn MetaCache>) -> Self {
        Self { meta_cache }
    }

    // 从mete-chain拉取对应device
    async fn search_from_meta(&self, object_id: &ObjectId) -> BuckyResult<NONObjectInfo> {
        let ret = self.meta_cache.get_object(object_id).await.map_err(|e| {
            let msg = format!(
                "load object from meta chain failed! obj={} err={}",
                object_id, e
            );
            error!("{}", msg);

            BuckyError::new(e, msg)
        })?;

        if ret.is_none() {
            warn!(
                "load object from meta chain but not found! obj={}",
                object_id,
            );
            return Err(BuckyError::from(BuckyErrorCode::NotFound));
        }

        let ret = ret.unwrap();

        let ret = NONObjectInfo::new(object_id.to_owned(), ret.object_raw, Some(ret.object));

        Ok(ret)
    }
}

#[async_trait]
impl ObjectSearcher for MetaSearcher {
    async fn search(
        &self,
        _owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo> {
        self.search_from_meta(&object_id).await
    }
}

struct NOCSearcher {
    noc: NamedObjectCacheRef,
    local_device_id: DeviceId,
}

impl NOCSearcher {
    pub fn new(noc: NamedObjectCacheRef, local_device_id: DeviceId) -> Self {
        Self {
            noc,
            local_device_id,
        }
    }

    // 直接从本地noc查询
    async fn search_from_noc(&self, object_id: &ObjectId) -> BuckyResult<NONObjectInfo> {
        let req = NamedObjectCacheGetObjectRequest {
            object_id: object_id.clone(),
            source: RequestSourceInfo::new_local_system(),
            last_access_rpath: None,
        };

        match self.noc.get_object(&req).await? {
            Some(info) => {
                Ok(info.object)
            }
            None => Err(BuckyError::from(BuckyErrorCode::NotFound)),
        }
    }
}

#[async_trait]
impl ObjectSearcher for NOCSearcher {
    async fn search(
        &self,
        _owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo> {
        self.search_from_noc(&object_id).await
    }
}

struct ZoneSearcher {
    zone_manager: ZoneManager,
    noc: NamedObjectCacheRef,
    bdt_stack: StackGuard,

    non_processor: AsyncMutex<Option<(DeviceId, NONOutputProcessorRef)>>,
}

impl ZoneSearcher {
    pub fn new(
        zone_manager: ZoneManager,
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
    ) -> Self {
        Self {
            zone_manager,
            noc,
            bdt_stack,
            non_processor: AsyncMutex::new(None),
        }
    }

    async fn create_requestor_to_ood(
        &self,
        ood_device_id: &DeviceId,
    ) -> BuckyResult<NONOutputProcessorRef> {
        let device = self
            .zone_manager
            .device_manager()
            .search(ood_device_id)
            .await?;

        let bdt_requestor = BdtHttpRequestor::new(
            self.bdt_stack.clone(),
            device,
            cyfs_base::NON_STACK_BDT_VPORT,
        );

        let requestor = RequestorWithRetry::new(
            Box::new(bdt_requestor),
            2,
            RequestorRetryStrategy::ExpInterval,
            Some(std::time::Duration::from_secs(60 * 10)),
        );

        let non = NONRequestor::new(None, Arc::new(Box::new(requestor)));
        Ok(non.into_processor())
    }

    async fn get_processor(&self, ood_device_id: &DeviceId) -> BuckyResult<NONOutputProcessorRef> {
        let mut current = self.non_processor.lock().await;

        // should check if ood changed
        if let Some(current) = &*current {
            if current.0 == *ood_device_id {
                return Ok(current.1.clone());
            }
        }

        let processor = self.create_requestor_to_ood(ood_device_id).await?;
        *current = Some((ood_device_id.to_owned(), processor.clone()));

        Ok(processor)
    }

    async fn search_from_ood(&self, object_id: &ObjectId) -> BuckyResult<NONObjectInfo> {
        let zone_info = self.zone_manager.get_current_info().await?;
        if zone_info.zone_role.is_ood_device() {
            return Err(BuckyError::from(BuckyErrorCode::NotFound));
        }

        if zone_info.zone_device_ood_id == *object_id {
            let msg = format!("zone searcher not support ood itself! id={}", object_id);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let non_processor = self.get_processor(&zone_info.zone_device_ood_id).await?;

        let req = NONGetObjectOutputRequest {
            common: NONOutputRequestCommon {
                req_path: None,
                dec_id: None,
                level: NONAPILevel::NON,

                // 用以处理默认行为
                target: Some(zone_info.zone_device_ood_id.object_id().clone()),

                flags: 0,
            },
            object_id: object_id.to_owned(),
            inner_path: None,
        };

        let resp = non_processor.get_object(req).await?;

        info!(
            "get object from current zone's ood: obj={}, ood={}",
            object_id, zone_info.zone_device_ood_id
        );

        let _ = self.update_noc(&resp.object).await;

        Ok(resp.object)
    }

    async fn update_noc(&self, object: &NONObjectInfo) -> BuckyResult<()> {
        let info = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_system(),
            object: object.clone(),
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: Some(AccessString::full_except_write().value()),
        };

        match self.noc.put_object(&info).await {
            Ok(resp) => {
                match resp.result {
                    NamedObjectCachePutObjectResult::AlreadyExists => {
                        info!("object already in noc: {}", object.object_id);
                    }
                    NamedObjectCachePutObjectResult::Merged => {
                        info!(
                            "object already in noc and signs updated: {}",
                            object.object_id
                        );
                    }
                    _ => {
                        info!("insert object to noc success: {}", object.object_id);
                    }
                }
                Ok(())
            }
            Err(e) => {
                error!("insert object to noc failed: {}", object.object_id);

                Err(e)
            }
        }
    }
}

#[async_trait]
impl ObjectSearcher for ZoneSearcher {
    async fn search(
        &self,
        _owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo> {
        self.search_from_ood(&object_id).await
    }
}

// 对象查找器，只从下面几个地方查找
// noc -> meta-chain -> zone's ood

#[derive(Clone)]
pub struct CompoundObjectSearcher {
    meta: ObjectSearcherRef,
    noc: ObjectSearcherRef,
    zone: Arc<OnceCell<ObjectSearcherRef>>,
}

impl CompoundObjectSearcher {
    pub fn new(
        noc: NamedObjectCacheRef,
        local_device_id: DeviceId,
        meta_cache: Box<dyn MetaCache>,
    ) -> Self {
        let ret = Self {
            noc: Arc::new(Box::new(NOCSearcher::new(noc, local_device_id))),
            meta: Arc::new(Box::new(MetaSearcher::new(meta_cache))),
            zone: Arc::new(OnceCell::new()),
        };

        ret
    }

    pub fn into_ref(self) -> ObjectSearcherRef {
        Arc::new(Box::new(self))
    }

    pub fn init_zone_searcher(
        &self,
        zone_manager: ZoneManager,
        noc: NamedObjectCacheRef,
        bdt_stack: StackGuard,
    ) {
        let zone: ObjectSearcherRef =
            Arc::new(Box::new(ZoneSearcher::new(zone_manager, noc, bdt_stack)));

        if let Err(_) = self.zone.set(zone) {
            unreachable!();
        }
    }
}

#[async_trait]
impl ObjectSearcher for CompoundObjectSearcher {
    async fn search(
        &self,
        owner_id: Option<ObjectId>,
        object_id: &ObjectId,
    ) -> BuckyResult<NONObjectInfo> {
        self.search_ex(owner_id, object_id, ObjectSearcherFlags::full())
            .await
    }

    async fn search_ex(
        &self,
        owner_id: Option<ObjectId>,
        object_id: &ObjectId,
        flags: u32,
    ) -> BuckyResult<NONObjectInfo> {
        if flags & OBJECT_SEARCH_FLAG_NOC != 0 {
            if let Ok(ret) = self.noc.search(owner_id.clone(), object_id).await {
                return Ok(ret);
            }
        }

        if flags & OBJECT_SEARCH_FLAG_META != 0 {
            if let Ok(ret) = self.meta.search(owner_id.clone(), object_id).await {
                return Ok(ret);
            }
        }

        if flags & OBJECT_SEARCH_FLAG_ZONE != 0 {
            if let Some(zone) = self.zone.get() {
                if let Ok(ret) = zone.search(owner_id, object_id).await {
                    return Ok(ret);
                }
            }
        }

        let msg = format!(
            "search object but not found: type={:?}, id={}, flags={}",
            object_id.obj_type_code(),
            object_id,
            flags,
        );
        warn!("{}", msg);

        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
    }
}
