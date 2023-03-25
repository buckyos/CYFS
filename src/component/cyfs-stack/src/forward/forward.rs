use super::fail_handler::HttpRequestorWithDeviceFailHandler;
use crate::meta::ObjectFailHandler;
use crate::resolver::DeviceCache;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use futures::future::{AbortHandle, Abortable};
use std::sync::Arc;

pub(crate) struct ForwardProcessorCreator {
    bdt_stack: StackGuard,
    device_manager: Box<dyn DeviceCache>,
    fail_handler: ObjectFailHandler,
}

impl ForwardProcessorCreator {
    pub fn new(bdt_stack: StackGuard, device_manager: Box<dyn DeviceCache>, fail_handler: ObjectFailHandler,) -> Self {
        Self {
            bdt_stack,
            device_manager,
            fail_handler,
        }
    }

    async fn create_requestor(&self, target: &DeviceId) -> BuckyResult<Box<dyn HttpRequestor>> {
        let device = self.device_manager.search(target).await.map_err(|e| {
            error!("get forward target device failed! device={}, {}", target, e);
            e
        })?;

        let bdt = BdtHttpRequestor::new(self.bdt_stack.clone(), device, cyfs_base::NON_STACK_BDT_VPORT);
        
        let bdt_with_fail_handler = HttpRequestorWithDeviceFailHandler::new(
            self.fail_handler.clone(),
            Box::new(bdt),
            target.clone(),
        );

        // FIXME An additional timeout here
        let requestor = RequestorWithRetry::new(
            Box::new(bdt_with_fail_handler),
            2,
            RequestorRetryStrategy::ExpInterval,
            Some(std::time::Duration::from_secs(60 * 10)),
        );

        Ok(Box::new(requestor))
    }

    async fn create_output_processor(
        &self,
        target: &DeviceId,
    ) -> BuckyResult<Arc<Box<dyn HttpRequestor>>> {
        // 转发到目标device
        let requestor = self.create_requestor(target).await?;

        Ok(Arc::new(requestor))
    }

    async fn create_forward_processor(
        &self,
        target: &DeviceId,
    ) -> BuckyResult<Arc<Box<dyn HttpRequestor>>> {
        let processor = self.create_output_processor(target).await?;

        Ok(processor)
    }
}

use lru_time_cache::{Entry, LruCache};

#[derive(Clone)]
struct CacheItemError {
    error: BuckyErrorCode,
    msg: String,
    tick: u64,
    error_count: u32,
}

#[derive(Clone)]
struct CacheItemPending {
    waker_list: Vec<AbortHandle>,
    error_count: u32,
}

#[derive(Clone)]
enum CacheItem {
    Value(HttpRequestorRef),
    Error(CacheItemError),
    Pending(CacheItemPending),
}

#[derive(Clone)]
pub(crate) struct ForwardRequestorContainer {
    list: Arc<Mutex<LruCache<DeviceId, CacheItem>>>,
    factory: Arc<ForwardProcessorCreator>,

    // 出错的缓存间隔，微秒
    error_cache_min_interval: u64,
    error_cache_max_interval: u64,
}

impl ForwardRequestorContainer {
    pub fn new(factory: Arc<ForwardProcessorCreator>) -> Self {
        let list = LruCache::with_expiry_duration(std::time::Duration::from_secs(60 * 15));

        Self {
            list: Arc::new(Mutex::new(list)),
            factory,
            error_cache_min_interval: 1000 * 1000 * 2,
            error_cache_max_interval: 1000 * 1000 * 60 * 2,
        }
    }

    fn cache_result(&self, device_id: &DeviceId, ret: &BuckyResult<HttpRequestorRef>) {
        let waker_list = {
            let mut list = self.list.lock().unwrap();
            let item = list.remove(&device_id).unwrap();
            let pending_item = if let CacheItem::Pending(pending_item) = item {
                pending_item
            } else {
                unreachable!();
            };

            let cache = match ret {
                Ok(v) => CacheItem::Value(v.clone()),
                Err(error) => CacheItem::Error(CacheItemError {
                    error: error.code(),
                    msg: error.msg().to_owned(),
                    tick: bucky_time_now(),
                    error_count: pending_item.error_count + 1,
                }),
            };

            list.insert(device_id.to_owned(), cache.clone());

            pending_item.waker_list
        };

        // 唤醒所有等待器
        for item in waker_list {
            item.abort();
        }
    }

    async fn create(&self, device_id: &DeviceId) -> BuckyResult<HttpRequestorRef> {
        let ret = self.factory.create_forward_processor(&device_id).await;
        self.cache_result(&device_id, &ret);

        ret
    }

    fn direct_get(&self, device_id: &DeviceId) -> BuckyResult<HttpRequestorRef> {
        let mut list = self.list.lock().unwrap();
        match list.get(device_id).unwrap() {
            CacheItem::Value(v) => Ok(v.clone()),
            CacheItem::Error(CacheItemError {
                error,
                msg,
                tick: _,
                error_count: _,
            }) => Err(BuckyError::new(error.to_owned(), msg.to_owned())),
            CacheItem::Pending(_) => {
                unreachable!();
            }
        }
    }

    fn cacl_next_timeout_on_error(&self, error_count: u32) -> u64 {
        let mut ret = self.error_cache_min_interval.pow(error_count + 1);
        if ret > self.error_cache_max_interval {
            ret = self.error_cache_max_interval;
        }

        ret
    }

    pub async fn get(&self, device_id: &DeviceId) -> BuckyResult<HttpRequestorRef> {
        let mut waker = None;
        {
            let mut list = self.list.lock().unwrap();
            match list.entry(device_id.to_owned()) {
                Entry::Occupied(o) => {
                    let v = o.into_mut();
                    match v {
                        CacheItem::Value(v) => {
                            debug!("got cache forward to device: {}", device_id);
                            return Ok(v.clone());
                        }
                        CacheItem::Error(CacheItemError {
                            error,
                            msg,
                            tick,
                            error_count,
                        }) => {
                            // 计算超时时间
                            let timeout = self.cacl_next_timeout_on_error(*error_count);
                            if bucky_time_now() - *tick > timeout {
                                warn!(
                                    "cache forward to device error timeout! device={}, timeout={}",
                                    device_id, timeout
                                );

                                let item = CacheItem::Pending(CacheItemPending {
                                    waker_list: Vec::new(),
                                    error_count: *error_count,
                                });
                                *v = item;
                            } else {
                                warn!(
                                    "forward to device still in error state: device={}, tick={}, error_count={}",
                                    device_id, tick, error_count,
                                );
                                return Err(BuckyError::new(error.to_owned(), msg.to_owned()));
                            }
                        }
                        CacheItem::Pending(CacheItemPending {
                            waker_list,
                            error_count: _,
                        }) => {
                            debug!(
                                "got cache forward to device still in creating: {}",
                                device_id
                            );

                            // 正在创建中，需要等待创建完毕
                            let (abort_handle, abort_registration) = AbortHandle::new_pair();
                            waker_list.push(abort_handle);
                            waker = Some(abort_registration);
                        }
                    }
                }
                Entry::Vacant(v) => {
                    info!("will create forward to device={}", device_id);
                    let item = CacheItem::Pending(CacheItemPending {
                        waker_list: Vec::new(),
                        error_count: 0,
                    });

                    v.insert(item);
                }
            }
        };

        if let Some(waker) = waker {
            // 等待创建完毕
            let future = Abortable::new(async_std::future::pending::<()>(), waker);
            future.await.unwrap_err();
            self.direct_get(device_id)
        } else {
            // 异步创建
            self.create(device_id).await
        }
    }

    pub fn gc(&self) {
        let device_id = DeviceId::default();
        let gc_list = {
            let mut list = self.list.lock().unwrap();
            let (_, ret) = list.notify_get(&device_id);
            if list.len() > 0 {
                info!("non forward to device alive count={}", list.len(),);
            }
            ret
        };

        for (key, _) in gc_list {
            info!("will gc router forward to device item: device={}", key,);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ForwardProcessorManager {
    container: ForwardRequestorContainer,
}

impl ForwardProcessorManager {
    pub fn new(bdt_stack: StackGuard, device_manager: Box<dyn DeviceCache>, fail_handler: ObjectFailHandler) -> Self {
        let creator = ForwardProcessorCreator::new(bdt_stack, device_manager, fail_handler);
        let container = ForwardRequestorContainer::new(Arc::new(creator));
        Self { container }
    }

    pub async fn get(&self, device_id: &DeviceId) -> BuckyResult<HttpRequestorRef> {
        self.container.get(device_id).await
    }

    pub fn start(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60)).await;

                this.container.gc();
            }
        });
    }
}
