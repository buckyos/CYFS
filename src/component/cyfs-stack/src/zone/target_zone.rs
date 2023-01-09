use super::zone_manager::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;

#[derive(Clone)]
pub struct TargetZoneInfo {
    pub is_current_zone: bool,
    pub target_device: DeviceId,
    pub target_ood: DeviceId,
}

pub struct TargetZoneManager {
    zone_manager: ZoneManagerRef,
    cache: Mutex<lru_time_cache::LruCache<ObjectId, TargetZoneInfo>>,
}

impl TargetZoneManager {
    pub fn new(zone_manager: ZoneManagerRef) -> Self {
        Self {
            zone_manager,
            cache: Mutex::new(lru_time_cache::LruCache::with_expiry_duration_and_capacity(
                std::time::Duration::from_secs(3600 * 24),
                512,
            )),
        }
    }

    // 解析目标device，target=None则指向当前zone的ood
    pub async fn resolve_target_with_strict_zone(
        &self,
        target: Option<&ObjectId>,
        object_raw: Option<Vec<u8>>,
    ) -> BuckyResult<(ZoneId, DeviceId)> {
        match target {
            None => {
                // 没有指定target，那么目标是当前zone和当前zone的ood device
                let info = self.zone_manager.get_current_info().await?;

                Ok((info.zone_id.clone(), info.zone_device_ood_id.clone()))
            }
            Some(target) => {
                let obj_type = target.obj_type_code();
                match obj_type {
                    ObjectTypeCode::Device => {
                        let zone = self.zone_manager.resolve_zone(target, object_raw).await?;
                        let device_id = target.try_into().unwrap();
                        Ok((zone.zone_id(), device_id))
                    }

                    ObjectTypeCode::People | ObjectTypeCode::Group => {
                        let zone = self.zone_manager.resolve_zone(target, object_raw).await?;
                        Ok((zone.zone_id(), zone.ood().clone()))
                    }

                    ObjectTypeCode::Custom => {
                        // 从object_id无法判断是不是zone类型，这里强制当作zone_id来查询一次
                        let zone_id = target.clone().try_into().map_err(|e| {
                            let msg =
                                format!("unknown custom target_id type! target={}, {}", target, e);
                            error!("{}", msg);
                            BuckyError::new(BuckyErrorCode::UnSupport, msg)
                        })?;

                        if let Some(zone) = self.zone_manager.query(&zone_id) {
                            Ok((zone_id, zone.ood().clone()))
                        } else {
                            let msg = format!("zone_id not found or invalid: {}", zone_id);
                            error!("{}", msg);

                            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                        }
                    }

                    _ => {
                        // 其余类型暂不支持
                        let msg = format!(
                            "search zone for object type not support: type={:?}, obj={}",
                            obj_type, target
                        );
                        error!("{}", msg);

                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                    }
                }
            }
        }
    }

    pub async fn resolve_target(&self, target: Option<&ObjectId>) -> BuckyResult<TargetZoneInfo> {
        if target.is_none() {
            let info = self.zone_manager.get_current_info().await?;
            return Ok(TargetZoneInfo {
                is_current_zone: true,
                target_device: info.zone_device_ood_id.clone(),
                target_ood: info.zone_device_ood_id.clone(),
            });
        }

        let target = target.unwrap();
        let ret = {
            let mut list = self.cache.lock().unwrap();
            list.get(target).cloned()
        };

        if ret.is_some() {
            return Ok(ret.unwrap());
        }

        let ret = self.resolve_target_zone(target).await?;
        {
            let mut list = self.cache.lock().unwrap();
            list.insert(target.to_owned(), ret.clone());
        }

        Ok(ret)
    }

    async fn resolve_target_zone(&self, target: &ObjectId) -> BuckyResult<TargetZoneInfo> {
        let info = self.zone_manager.get_current_info().await?;

        let obj_type = target.obj_type_code();
        match obj_type {
            ObjectTypeCode::Device => {
                let device_id = target.clone().try_into().unwrap();
                let device = self
                    .zone_manager
                    .device_manager()
                    .search(&device_id)
                    .await?;

                match device.desc().owner() {
                    None => {
                        // arphan zone
                        if device_id == info.device_id {
                            let ret = TargetZoneInfo {
                                is_current_zone: true,
                                target_device: device_id.clone(),
                                target_ood: device_id,
                            };
                            Ok(ret)
                        } else {
                            let ret = TargetZoneInfo {
                                is_current_zone: false,
                                target_device: device_id.clone(),
                                target_ood: device_id,
                            };
                            Ok(ret)
                        }
                    }
                    Some(owner) => {
                        if *owner == info.owner_id {
                            let ret = TargetZoneInfo {
                                is_current_zone: true,
                                target_device: device_id.clone(),
                                target_ood: info.zone_device_ood_id.clone(),
                            };

                            Ok(ret)
                        } else {
                            // other zone
                            // search device's owner and zone
                            let mut target = target.to_owned();
                            let (_mode, mut ood_list) = self
                                .zone_manager
                                .search_zone_ood_by_owner(&mut target, None)
                                .await?;
                            assert!(ood_list.len() > 0);

                            let ret = TargetZoneInfo {
                                is_current_zone: false,
                                target_device: device_id.clone(),
                                target_ood: ood_list.remove(0),
                            };
                            Ok(ret)
                        }
                    }
                }
            }

            ObjectTypeCode::People | ObjectTypeCode::Group => {
                if info.owner_id == *target {
                    let ret = TargetZoneInfo {
                        is_current_zone: true,
                        target_device: info.zone_device_ood_id.clone(),
                        target_ood: info.zone_device_ood_id.clone(),
                    };

                    Ok(ret)
                } else {
                    // other zone
                    let mut target = target.to_owned();
                    let (_mode, mut ood_list) = self
                        .zone_manager
                        .search_zone_ood_by_owner(&mut target, None)
                        .await?;
                    assert!(ood_list.len() > 0);

                    let ood = ood_list.remove(0);
                    let ret = TargetZoneInfo {
                        is_current_zone: false,
                        target_device: ood.clone(),
                        target_ood: ood,
                    };
                    Ok(ret)
                }
            }

            ObjectTypeCode::Custom => {
                // 从object_id无法判断是不是zone类型，这里强制当作zone_id来查询一次
                let zone_id = target.clone().try_into().map_err(|e| {
                    let msg = format!("unknown custom target_id type! target={}, {}", target, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::UnSupport, msg)
                })?;

                if zone_id == info.zone_id {
                    let ret = TargetZoneInfo {
                        is_current_zone: true,
                        target_device: info.zone_device_ood_id.clone(),
                        target_ood: info.zone_device_ood_id.clone(),
                    };

                    Ok(ret)
                } else {
                    if let Some(zone) = self.zone_manager.query(&zone_id) {
                        let ret = TargetZoneInfo {
                            is_current_zone: false,
                            target_device: zone.ood().clone(),
                            target_ood: zone.ood().clone(),
                        };
                        Ok(ret)
                    } else {
                        let msg = format!("zone_id not found or invalid: {}", zone_id);
                        error!("{}", msg);

                        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                    }
                }
            }

            _ => {
                // 其余类型暂不支持
                let msg = format!(
                    "search zone for object type not support: type={:?}, obj={}",
                    obj_type, target
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}
