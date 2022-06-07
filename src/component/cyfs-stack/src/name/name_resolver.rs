use super::name_cache::*;
use crate::meta::MetaCache;
use cyfs_base::{
    bucky_time_now, BuckyError, BuckyErrorCode, BuckyResult, NameInfo, NameLink, NameState,
    ObjectId,
};
use cyfs_lib::*;

use cyfs_debug::Mutex;
use futures::future::{AbortHandle, Abortable};
use std::collections::{hash_map::Entry, HashMap};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;


// name缓存超时时间，默认一天
// const NAME_CACHE_TIMEOUT_IN_MICRO_SECS: u64 = 1000 * 1000 * 60 * 60 * 24;
const NAME_CACHE_TIMEOUT_IN_MICRO_SECS: u64 = 1000 * 1000 * 60; // 暂时改为一分钟

// name查询但不存在的超时时间，默认一小时
//const NAME_CACHE_NOT_FOUND_TIMEOUT_IN_MICRO_SECS: u64 = 1000 * 1000 * 60 * 60;
const NAME_CACHE_NOT_FOUND_TIMEOUT_IN_MICRO_SECS: u64 = 1000 * 1000 * 60; // 暂时改为一分钟

// 查询出错重试的最大时长，共享同一个
// const NAME_CACHE_ERROR_MAX_RETRY_INTERVAL_IN_MICRO_SECS: u64 = 1000 * 1000 * 60 * 60;
const NAME_CACHE_ERROR_MAX_RETRY_INTERVAL_IN_MICRO_SECS: u64 = 1000 * 1000 * 60; // 暂时改为一分钟

// 递归解析的最大深度
const NAME_RESOLVE_MAX_DEPTH: u8 = 8;

pub enum NameResult {
    ObjectLink(ObjectId),
    IPLink(IpAddr),
}

enum LookupResult {
    Link(NameLink),
    Continue(()),
}

struct NameResolvingItem {
    wait_list: Vec<AbortHandle>,
    // result: Option<BuckyResult<Option<(NameInfo, NameState)>>>,
}

#[derive(Clone)]
pub struct NameResolver {
    cache: NOCCollectionSync<NameCache>,
    meta_cache: Arc<Box<dyn MetaCache>>,

    resolving_list: Arc<Mutex<HashMap<String, NameResolvingItem>>>,

    next_retry_interval: Arc<AtomicU64>,
}

impl NameResolver {
    pub fn new(meta_cache: Box<dyn MetaCache>, noc: Box<dyn NamedObjectCache>) -> Self {
        let id = "cyfs-name-cache";
        Self {
            meta_cache: Arc::new(meta_cache),
            cache: NOCCollectionSync::new(id, noc),
            resolving_list: Arc::new(Mutex::new(HashMap::new())),
            next_retry_interval: Arc::new(AtomicU64::new(1000 * 1000 * 2)),
        }
    }

    pub async fn start(&self) -> BuckyResult<()> {
        // 首先从noc里面加载已经缓存的数据
        if let Err(e) = self.cache.load().await {
            if e.code() != BuckyErrorCode::InvalidFormat {
                error!("load name cache from noc error: {}", e);
                return Err(e);
            }

            // 如果加载到了不可识别的数据，说明版本出问题了，这里直接重置整个对象
            warn!(
                "load name cache from noc but unrecognizable data! now will clear all {}",
                e
            );
            self.cache.set_dirty(true);
            self.cache.async_save();
        }

        self.cache
            .start_save(std::time::Duration::from_secs(60 * 5));
        Ok(())
    }

    pub async fn lookup(&self, name: &str) -> BuckyResult<NameResult> {
        self.recursive_call(name, true).await
    }

    pub async fn resolve(&self, name: &str) -> BuckyResult<NameResult> {
        self.recursive_call(name, false).await
    }

    async fn recursive_call(&self, name: &str, lookup: bool) -> BuckyResult<NameResult> {
        // 记录解析的深度，为了避免name链接出现环，所以这里我们要加一个最大深度限制
        let mut cur_depth: u8 = 0;

        let mut name = name.to_owned();

        // 递归拆解为循环来解析
        loop {
            let ret = if lookup {
                self.lookup_impl(&name).await
            } else {
                self.resolve_impl(&name).await
            }?;

            match ret {
                LookupResult::Link(link) => {
                    match link {
                        NameLink::ObjectLink(id) => {
                            break Ok(NameResult::ObjectLink(id));
                        }
                        NameLink::IPLink(addr) => {
                            break Ok(NameResult::IPLink(addr));
                        }
                        NameLink::OtherNameLink(other_name) => {
                            // 继续解析

                            // 不能自己链接到自己
                            if other_name == name {
                                let msg = format!("name link to self: name={}", name);
                                error!("{}", msg);

                                break Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                            }

                            cur_depth += 1;
                            if cur_depth >= NAME_RESOLVE_MAX_DEPTH {
                                let msg = format!(
                                    "name link extent max limit: name={}, depth={}",
                                    name, cur_depth
                                );
                                error!("{}", msg);

                                break Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
                            }

                            // 继续解析链接到的下一级name
                            name = other_name;
                            continue;
                        }
                    }
                }
                LookupResult::Continue(_) => {
                    // 收到解析完毕通知后，继续一轮lookup_impl
                }
            }
        }
    }

    // 首先从缓存里面查找
    async fn lookup_impl(&self, name: &str) -> BuckyResult<LookupResult> {
        {
            let mut data = self.cache.coll().lock().unwrap();
            let item = data.get(name);
            if self.check_timeout(&item) {
                item.reset(name);
            }

            match item.status {
                NameItemStatus::Ready => {
                    if let Some(ref link) = item.link {
                        return Ok(LookupResult::Link(link.clone()));
                    } else {
                        unreachable!("name item link should not empty! name={}", name);
                    }
                }
                NameItemStatus::NotFound => {
                    let msg = format!(
                        "lookup name but not found: name={}, tick={}",
                        name, item.last_tick
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }
                NameItemStatus::Error => {
                    let msg = format!(
                        "lookup name but got error: name={}, tick={}",
                        name, item.last_tick
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InternalError, msg));
                }
                NameItemStatus::Init => {
                    // assert!(item.link.is_none());

                    // 向meta发起解析
                }
            }
        }

        self.resolve_from_meta(name).await;

        Ok(LookupResult::Continue(()))
    }

    fn check_timeout(&self, item: &NameCacheItem) -> bool {
        let now = bucky_time_now();
        if item.status == NameItemStatus::Ready {
            assert!(item.last_tick > 0);
            if now - item.last_tick >= NAME_CACHE_TIMEOUT_IN_MICRO_SECS {
                return true;
            }
        } else if item.status == NameItemStatus::NotFound {
            assert!(item.last_tick > 0);
            if now - item.last_tick >= NAME_CACHE_NOT_FOUND_TIMEOUT_IN_MICRO_SECS {
                return true;
            }
        } else if item.last_resolve_status == NameItemStatus::Error {
            assert!(item.last_tick > 0);
            let interval = self.next_retry_interval.load(Ordering::SeqCst);
            if now - item.last_tick >= interval {
                return true;
            }
        }

        false
    }

    async fn resolve_impl(&self, name: &str) -> BuckyResult<LookupResult> {
        // 首先从meta解析，meta解析内部会更新缓存，再从缓存读取结果(last_resolve_status)
        self.resolve_from_meta(name).await;

        let mut data = self.cache.coll().lock().unwrap();
        let item = data.get(name);

        // 这里只判断last_resolve_status
        // 如果缓存里面有结果，但是从meta解析失败了，那么也要认为resolve失败
        // 这种情况下status=NotFound/Ready,但last_resolve_status=Error
        match item.last_resolve_status {
            NameItemStatus::Ready => {
                if let Some(ref link) = item.link {
                    Ok(LookupResult::Link(link.clone()))
                } else {
                    unreachable!("name item link should not empty! name={}", name);
                }
            }
            NameItemStatus::NotFound => {
                let msg = format!(
                    "lookup name still in not found state: name={}, tick={}",
                    name, item.last_tick
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
            NameItemStatus::Error => {
                let msg = format!(
                    "lookup name still in error state: name={}, tick={}",
                    name, item.last_tick
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InternalError, msg))
            }
            NameItemStatus::Init => {
                unreachable!();
            }
        }
    }

    async fn resolve_from_meta(&self, name: &str) {
        info!("will resolve name: {}", name);

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        {
            let mut list = self.resolving_list.lock().unwrap();
            match list.entry(name.to_owned()) {
                Entry::Vacant(v) => {
                    let item = NameResolvingItem {
                        wait_list: vec![abort_handle],
                    };
                    v.insert(item);

                    let this = self.clone();
                    let name = name.to_owned();
                    async_std::task::spawn(async move {
                        this.resolve_from_meta_impl(&name).await;
                    });
                }
                Entry::Occupied(mut o) => {
                    let item = o.get_mut();
                    assert!(!item.wait_list.is_empty());
                    item.wait_list.push(abort_handle);
                }
            };
        }

        let _ = Abortable::new(async_std::future::pending::<()>(), abort_registration).await;
    }

    async fn resolve_from_meta_impl(&self, name: &str) {
        let dur = std::time::Duration::from_secs(30);
        let result;
        match async_std::future::timeout(dur, self.meta_cache.get_name(name)).await {
            Ok(ret) => {
                result = ret;
            }
            Err(async_std::future::TimeoutError { .. }) => {
                error!("get name from meta cache timeout: name={}", name);
                result = Err(BuckyError::from(BuckyErrorCode::Timeout));
            }
        };

        // 更新缓存
        self.update_resolve_result(name, result);
        self.cache.set_dirty(true);

        // 触发通知
        let notify_list = self.resolving_list.lock().unwrap().remove(name).unwrap();
        assert!(notify_list.wait_list.len() > 0);

        for item in notify_list.wait_list {
            item.abort();
        }
    }

    fn update_resolve_result(&self, name: &str, ret: BuckyResult<Option<(NameInfo, NameState)>>) {
        let mut data = self.cache.coll().lock().unwrap();
        let item = data.get(name);
        // 有可能不为none，比如强制向meta发起了解析
        // assert!(item.link.is_none());

        // 更新最后一次操作的时间戳
        item.last_tick = bucky_time_now();

        match ret {
            Ok(None) => {
                info!("resolve name from meta but not found: name={}", name);
                item.status = NameItemStatus::NotFound;
                item.last_resolve_status = NameItemStatus::NotFound;
            }
            Ok(Some(value)) => {
                item.status = NameItemStatus::Ready;
                item.last_resolve_status = NameItemStatus::Ready;

                let link = value.0.record.link;
                info!(
                    "resolve name from meta success: name={}, current={:?}, new={:?}",
                    name, item.link, link
                );
                item.link = Some(link);
            }
            Err(e) => {
                info!(
                    "resolve name from meta error: name={}, status={}, {}",
                    name, item.status, e
                );

                item.last_resolve_status = NameItemStatus::Error;
                if item.status == NameItemStatus::Init {
                    item.status = NameItemStatus::Error;
                } else {
                    // 如果已经存在非错误的缓存结果，那么先不覆盖，保留缓存
                }

                // 增加全局的重试间隔
                let mut interval = self.next_retry_interval.load(Ordering::SeqCst);
                interval *= 2;
                if interval >= NAME_CACHE_ERROR_MAX_RETRY_INTERVAL_IN_MICRO_SECS {
                    interval = NAME_CACHE_ERROR_MAX_RETRY_INTERVAL_IN_MICRO_SECS;
                }
                self.next_retry_interval.store(interval, Ordering::SeqCst);
            }
        }
    }
}
