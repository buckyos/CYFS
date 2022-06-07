use super::super::manager::AclMatchInstanceRef;
use super::super::request::*;
use super::creator::AclRelationFactory;
use super::desc::*;
use super::AclSpecifiedRelationRef;
use cyfs_base::*;
use cyfs_debug::Mutex;

use lru_time_cache::{Entry, LruCache};
use std::sync::{Arc, RwLock};


#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AclRelationCacheKey {
    device_id: Option<DeviceId>,
    description: AclRelationDescription,
}

impl AclRelationCacheKey {
    pub fn new(desc: &AclRelationDescription, req: &dyn AclRequest) -> Self {
        match desc.who {
            AclRelationWho::My => Self {
                device_id: None,
                description: desc.to_owned(),
            },
            _ => Self {
                device_id: Some(req.device().to_owned()),
                description: desc.to_owned(),
            },
        }
    }
}

pub(crate) struct AclRelationCache {
    pub desc: AclRelationDescription,
    pub relation: RwLock<Option<BuckyResult<AclSpecifiedRelationRef>>>,
}


impl AclRelationCache {
    fn new(desc: &AclRelationDescription) -> Self {
        Self {
            desc: desc.to_owned(),
            relation: RwLock::new(None),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.relation.read().unwrap().is_some()
    }

    fn gc(&self) {
        info!("will gc acl relation cache: {:?}", self.desc);
        let mut v = self.relation.write().unwrap();
        *v = None;
    }
}

pub(crate) type AclRelationCacheRef = Arc<AclRelationCache>;

#[derive(Clone)]
struct AclRelationCacheHolder {
    cache: AclRelationCacheRef,
}

// lru_cache回收时候会触发这里的drop
impl Drop for AclRelationCacheHolder {
    fn drop(&mut self) {
        self.cache.gc();
    }
}

#[derive(Clone)]
pub(crate) struct AclRelationContainer {
    list: Arc<Mutex<LruCache<AclRelationCacheKey, AclRelationCacheHolder>>>,
    factory: AclRelationFactory,
}

impl AclRelationContainer {
    pub fn new(match_instance: AclMatchInstanceRef) -> Self {
        let list = LruCache::with_expiry_duration(std::time::Duration::from_secs(60 * 60));

        Self {
            list: Arc::new(Mutex::new(list)),
            factory: AclRelationFactory::new(match_instance),
        }
    }

    pub async fn get(
        &self,
        desc: &AclRelationDescription,
        req: &dyn AclRequest,
        prev_cache: Option<AclRelationCacheRef>,
    ) -> AclRelationCacheRef {
        let key = AclRelationCacheKey::new(desc, req);
        let cache = {
            let mut list = self.list.lock().unwrap();
            match list.entry(key) {
                Entry::Occupied(o) => o.into_mut().cache.clone(),
                Entry::Vacant(v) => {
                    // 如果存在老的cache，那么必须复用，不能创建新的
                    let cache = match prev_cache {
                        Some(cache) => cache,
                        None => Arc::new(AclRelationCache::new(desc)),
                    };
                    
                    let holder = AclRelationCacheHolder {
                        cache: cache.clone(),
                    };
                    v.insert(holder);
                    cache
                }
            }
        };

        // 尝试初始化
        // TODO 同一个desc并发初始化需要增加锁
        if cache.relation.read().unwrap().is_none() {
            let ret = self
                .factory
                .new_relation(desc, req)
                .await
                .map(|v| Arc::new(v));

            *cache.relation.write().unwrap() = Some(ret);
        }

        cache.clone()
    }

    pub fn gc(&self) {
        let random = AclRelationCacheKey {
            device_id: None,
            description: AclRelationDescription {
                who: AclRelationWho::My,
                what: AclRelationWhat::Device,
                category: AclRelationCategory::Device,
            },
        };

        let gc_list = {
            let mut list = self.list.lock().unwrap();
            let (_, ret) = list.notify_get(&random);
            if list.len() > 0 {
                info!("acl relation alive count={}", list.len(),);
            }
            ret
        };

        for (key, _) in gc_list {
            info!(
                "will gc acl relation item: desc={:?}, device={:?}",
                key.description, key.device_id
            );
        }
    }
}
