
use std::{sync::{RwLock, Arc}, collections::BTreeMap};

use cyfs_base::DeviceId;

struct DeviceStatusImpl {
    // status: StatusKind,
}

struct DeviceStatisticImpl {
    status: RwLock<DeviceStatusImpl>,
}

#[derive(Clone)]
pub struct DeviceStatistic(Arc<DeviceStatisticImpl>);

impl DeviceStatistic {
    pub fn new() -> Self {
        Self(Arc::new(DeviceStatisticImpl{
            status: RwLock::new(DeviceStatusImpl{

            }),
        }))
    }
}

struct DeviceStatisticManagerImpl {
    devices: RwLock<BTreeMap<DeviceId, DeviceStatistic>>
}

pub struct DeviceStatisticManager(Arc<DeviceStatisticManagerImpl>);

impl std::default::Default for DeviceStatisticManager {
    fn default() -> Self {
        Self(Arc::new(DeviceStatisticManagerImpl{
            devices: RwLock::new(BTreeMap::new()),
        }))
    }
}

impl DeviceStatisticManager {
    pub fn create_statistic(&self, id: DeviceId) -> DeviceStatistic {
        let devices = &mut *self.0.devices.write().unwrap();
        match devices.get(&id) {
            Some(v) => v.clone(),
            None => {
                let statistic = DeviceStatistic::new();
                devices.insert(id, v.clone());
                v
            }
        }
    }

}
