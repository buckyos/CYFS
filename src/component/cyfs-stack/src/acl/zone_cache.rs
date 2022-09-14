use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_core::ZoneObj;
use cyfs_lib::*;

use int_enum::IntEnum;
use std::collections::{hash_map, HashMap};

// 黑名单时长
const DEVICE_BLOCK_INTERVAL: u64 = 1000 * 1000 * 60 * 60 * 2;

#[repr(u16)]
#[derive(Debug, Eq, PartialEq, Copy, Clone, IntEnum)]
enum DeviceZonePos {
    Unknown = 0,
    CurZoneDevice = 1,
    CurZoneOOD = 2,
    OtherZoneDevice = 3,
    OtherZoneOOD = 4,
}

impl Into<u16> for DeviceZonePos {
    fn into(self) -> u16 {
        unsafe { std::mem::transmute(self as u16) }
    }
}

impl From<u16> for DeviceZonePos {
    fn from(code: u16) -> Self {
        match DeviceZonePos::from_int(code) {
            Ok(code) => code,
            Err(e) => {
                error!("unknown device zone code: {} {}", code, e);
                DeviceZonePos::Unknown
            }
        }
    }
}

#[derive(RawEncode, RawDecode, Clone)]
struct DeviceCacheItem {
    first_access: u64,
    access_acount: u32,
    zone_pos: u16,
    err: u16,
}

#[derive(Clone)]
struct DeviceCacheList {
    list: NOCCollectionSync<HashMap<String, DeviceCacheItem>>,
}

impl DeviceCacheList {
    pub fn new(noc: NamedObjectCacheRef, device_id: &DeviceId) -> Self {
        // 需要用device_id加以区分，避免出现更换device后复用数据库导致的不一致问题
        let id = format!("access-control-device-list-{}", device_id.to_string());
        let list = NOCCollectionSync::new(&id, noc);
        Self { list }
    }

    async fn init(&self) {
        if let Err(e) = self.list.load().await {
            error!("load device block list from noc failed! {}", e);
        }

        self.list.start_save(std::time::Duration::from_secs(60));
    }

    pub fn get_zone_pos(&self, device_id: &DeviceId) -> Option<BuckyResult<DeviceZonePos>> {
        // debug!("will get zone pos for device: {}", device_id);

        let device_id = device_id.to_string();

        let mut list = self.list.coll().lock().unwrap();

        match list.get_mut(&device_id) {
            Some(item) => {
                if item.err == 0 {
                    let pos = DeviceZonePos::from(item.zone_pos);
                    assert!(pos != DeviceZonePos::Unknown);
                    Some(Ok(pos))
                } else {
                    assert!(item.zone_pos == 0);
                    if bucky_time_now() - item.first_access >= DEVICE_BLOCK_INTERVAL {
                        warn!("device block timeout, now will remove from block list: device={}, first_access={}", device_id, item.first_access);
                        drop(item);
                        list.remove(&device_id);
                        self.list.set_dirty(true);
                        None
                    } else {
                        item.access_acount += 1;
                        Some(Err(BuckyError::from(BuckyErrorCode::from(item.err))))
                    }
                }
            }
            None => None,
        }
    }

    pub fn update(&self, device_id: &DeviceId, err: BuckyErrorCode, pos: DeviceZonePos) {
        let device_id = device_id.to_string();
        let mut list = self.list.coll().lock().unwrap();
        match list.entry(device_id.clone()) {
            hash_map::Entry::Vacant(v) => {
                v.insert(DeviceCacheItem {
                    first_access: bucky_time_now(),
                    access_acount: 1,
                    err: err.into(),
                    zone_pos: pos.into(),
                });
                info!(
                    "first cache acl device: {}, err={}, pos={:?}",
                    device_id, err, pos
                );
                self.list.set_dirty(true);
            }
            hash_map::Entry::Occupied(mut o) => {
                let item = o.get_mut();

                warn!(
                    "ac device cache changed: {}, old={},{:?}, new={},{:?}",
                    device_id, item.err, item.zone_pos, err, pos,
                );

                // 发生了竞争
                if err == BuckyErrorCode::Ok {
                    item.err = err.into();
                    item.zone_pos = pos.into();
                } else {
                    // 不再尝试更新
                }
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct LocalZoneCache {
    zone_manager: ZoneManagerRef,
    device_cache_list: DeviceCacheList,

    // 当前设备id
    current_device_id: DeviceId,
}

impl LocalZoneCache {
    pub fn new(zone_manager: ZoneManagerRef, noc: NamedObjectCacheRef) -> Self {
        let current_device_id = zone_manager.get_current_device_id().to_owned();
        let device_cache_list = DeviceCacheList::new(noc, &current_device_id);
        Self {
            current_device_id,
            zone_manager,
            device_cache_list,
        }
    }

    pub async fn init(&self) {
        self.device_cache_list.init().await;
    }

    pub async fn is_current_zone_device(&self, device_id: &DeviceId) -> BuckyResult<bool> {
        if *device_id == self.current_device_id {
            return Ok(true);
        }

        let pos = self.get_device_zone_pos(device_id).await?;
        let ret = match pos {
            DeviceZonePos::CurZoneDevice | DeviceZonePos::CurZoneOOD => true,
            DeviceZonePos::OtherZoneDevice | DeviceZonePos::OtherZoneOOD => false,
            DeviceZonePos::Unknown => unreachable!(),
        };

        Ok(ret)
    }

    async fn get_device_zone_pos(&self, device_id: &DeviceId) -> BuckyResult<DeviceZonePos> {
        // 首先从缓存查询
        match self.device_cache_list.get_zone_pos(device_id) {
            Some(Ok(pos)) => Ok(pos),
            Some(Err(e)) => {
                warn!("device already in block list: {}, {}", device_id, e);
                Err(e)
            }
            None => self.update_device_zone_pos(device_id).await,
        }
    }

    async fn update_device_zone_pos(&self, device_id: &DeviceId) -> BuckyResult<DeviceZonePos> {
        trace!("will update device zone pos: {}", device_id);
        let ret = self.search_device_zone_pos(device_id).await;
        info!("device zone pos updated: {}, ret={:?}", device_id, ret);

        match &ret {
            Ok(pos) => {
                self.device_cache_list
                    .update(device_id, BuckyErrorCode::Ok, pos.clone());
            }
            Err(e) => {
                self.device_cache_list
                    .update(device_id, e.code(), DeviceZonePos::Unknown);
            }
        };

        ret
    }

    async fn search_device_zone_pos(&self, device_id: &DeviceId) -> BuckyResult<DeviceZonePos> {
        // 直接从zone对象检查device_id是否存在
        let current_zone = self.zone_manager.get_current_zone().await?;

        if current_zone.is_ood(device_id) {
            return Ok(DeviceZonePos::CurZoneOOD);
        } else if current_zone.is_known_device(device_id) {
            return Ok(DeviceZonePos::CurZoneDevice);
        }

        let zone = self.zone_manager.get_zone(device_id, None).await?;
        if zone.zone_id() == current_zone.zone_id() {
            if current_zone.is_ood(device_id) {
                Ok(DeviceZonePos::CurZoneOOD)
            } else {
                Ok(DeviceZonePos::CurZoneDevice)
            }
        } else {
            if zone.is_ood(device_id) {
                Ok(DeviceZonePos::OtherZoneOOD)
            } else {
                Ok(DeviceZonePos::OtherZoneDevice)
            }
        }
    }
}
