use cyfs_base::*;
use cyfs_core::*;

use lru_time_cache::{Entry, LruCache};
use std::sync::Arc;
use cyfs_debug::Mutex;

/*
用以缓存下面两种zone信息：
1. 对于device，无法读取其owner(owner没上链等情况)，签名无效等错误原因，导致无法得到正确的zone，只能使用单device的孤儿zone
2. 对于owner(people/simplegroup)，由于获取不到，或者校验失败等原因，无法得到所属zone的ood，以错误处理
*/

struct OrphanZone {
    zone_id: ZoneId,
    zone: Zone,
}

struct OrphanZoneCache {
    device_list: LruCache<DeviceId, OrphanZone>,
    zone_list: LruCache<ZoneId, DeviceId>,
}

impl OrphanZoneCache {
    pub fn new(timeout: std::time::Duration) -> Self {
        Self {
            device_list: LruCache::with_expiry_duration(timeout.clone()),
            zone_list: LruCache::with_expiry_duration(timeout),
        }
    }

    pub fn get_by_device(&mut self, device_id: &DeviceId) -> Option<Zone> {
        let ret = self.device_list.get(device_id);
        match ret {
            Some(v) => {
                self.zone_list.get(&v.zone_id);
                Some(v.zone.clone())
            }
            None => None,
        }
    }

    pub fn get(&mut self, zone_id: &ZoneId) -> Option<Zone> {
        let ret = self.zone_list.get(zone_id);
        match ret {
            Some(device_id) => self.device_list.get(device_id).map(|v| v.zone.clone()),
            None => None,
        }
    }

    pub fn remove(&mut self, zone_id: &ZoneId) -> Option<Zone> {
        match self.zone_list.remove(zone_id) {
            Some(device_id) => self.device_list.remove(&device_id).map(|value| value.zone),
            None => None,
        }
    }
}

#[derive(Clone)]
pub(super) struct ZoneFailedCache {
    device_cache: Arc<Mutex<OrphanZoneCache>>,
    owner_cache: Arc<Mutex<LruCache<ObjectId, BuckyError>>>,
}

impl ZoneFailedCache {
    pub fn new() -> Self {
        // TODO 所有错误缓存一个小时
        let timeout = std::time::Duration::from_secs(60 * 60);

        Self {
            device_cache: Arc::new(Mutex::new(OrphanZoneCache::new(timeout.clone()))),
            owner_cache: Arc::new(Mutex::new(LruCache::with_expiry_duration(timeout))),
        }
    }

    // 获取指定device的孤儿zone
    pub fn get_orphan_zone(&self, device_id: &DeviceId) -> Option<Zone> {
        let mut cache = self.device_cache.lock().unwrap();
        cache.get_by_device(device_id)
    }

    // 直接获取对应的zone
    pub fn query_zone(&self, zone_id: &ZoneId) -> Option<Zone> {
        let mut cache = self.device_cache.lock().unwrap();
        cache.get(zone_id)
    }

    // remove the target zone from cache
    pub fn remove_zone(&self, zone_id: &ZoneId) -> Option<Zone> {
        let mut cache = self.device_cache.lock().unwrap();
        cache.remove(zone_id)
    }

    pub fn get_failed_owner(&self, object_id: &ObjectId) -> Option<BuckyError> {
        let mut cache = self.owner_cache.lock().unwrap();
        cache.get(object_id).map(|v| v.clone())
    }

    // device搜寻zone失败，创建孤儿zone并缓存
    pub fn on_device_zone_failed(&self, device_id: &DeviceId) -> Zone {
        let mut cache = self.device_cache.lock().unwrap();
        match cache.device_list.entry(device_id.to_owned()) {
            Entry::Occupied(v) => {
                warn!("zone failed from device already in cache! id={}", device_id);
                let v = v.into_mut();
                let zone_id = v.zone_id.clone();
                let zone = v.zone.clone();
                drop(v);
                cache.zone_list.get(&zone_id);
                zone
            }
            Entry::Vacant(v) => {
                // 创建对应的孤儿zone
                let (zone_id, zone) = Self::create_orphan_zone(device_id);
                let info = OrphanZone {
                    zone_id: zone_id.clone(),
                    zone: zone.clone(),
                };
                v.insert(info);
                cache.zone_list.insert(zone_id, device_id.to_owned());
                zone
            }
        }
    }

    // 搜寻或者校验owner失败，缓存对应的错误
    pub fn on_owner_failed(&self, owner: &ObjectId, err: BuckyError) {
        let mut cache = self.owner_cache.lock().unwrap();
        match cache.entry(owner.to_owned()) {
            Entry::Occupied(_v) => {
                warn!("zone failed from owner already in cache! id={}", owner);
            }
            Entry::Vacant(v) => {
                v.insert(err);
            }
        }
    }

    // 创建孤儿zone，也即owner本身就是device的zone
    fn create_orphan_zone(device_id: &DeviceId) -> (ZoneId, Zone) {
        let owner = device_id.object_id().to_owned();
        let zone = Zone::create(
            owner,
            OODWorkMode::Standalone,
            vec![device_id.to_owned()],
            vec![],
        );
        let zone_id: ZoneId = zone.desc().calculate_id().try_into().unwrap();

        info!(
            "create new orphan zone for device={}, zone={}",
            device_id, zone_id
        );

        (zone_id, zone)
    }
}
