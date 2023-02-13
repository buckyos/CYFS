use crate::SERVICE_MANAGER;
use crate::config::{ServiceState, ServiceConfig, DeviceConfigManager};
use crate::service::ServicePackageLocalState;

use cyfs_base::bucky_time_now;
use serde::Serialize;
use std::sync::Mutex;


#[derive(Serialize)]
pub struct OODServiceStatusItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enable: bool,
    pub target_state: ServiceState,
    pub package_state: ServicePackageLocalState,
    pub process_state: ServiceState,
}

#[derive(Serialize)]
pub struct OODDaemonStatus {
    last_update_time: u64,
    device_config: Vec<ServiceConfig>,
    services: Vec<OODServiceStatusItem>,
}

impl Default for OODDaemonStatus {
    fn default() -> Self {
        Self {
            last_update_time: bucky_time_now(),
            device_config: vec![],
            services: vec![],
        }
    }
}

struct OODDaemonStatusCacheItem {
    status: OODDaemonStatus,
    encoded: Option<serde_json::Value>,
}

pub struct OODDaemonStatusGenerator {
    status: Mutex<OODDaemonStatusCacheItem>,
}

impl OODDaemonStatusGenerator {
    pub fn new() -> Self {
        let cache = OODDaemonStatusCacheItem {
            status: OODDaemonStatus::default(),
            encoded: None,
        };

        Self {
            status: Mutex::new(cache),
        }
    }

    pub fn refresh_status(&self) -> Option<serde_json::Value> {
        let mut cache = self.status.lock().unwrap();
        if cache.encoded.is_none() || bucky_time_now() - cache.status.last_update_time > 1000 * 1000 * 5 {
            debug!("will refresh service status");
            
            cache.status = Self::gen();
            cache.encoded = Some(serde_json::to_value(&cache.status).unwrap());

            Some(cache.encoded.as_ref().unwrap().clone())
        } else {
            None
        }
    }

    fn gen() -> OODDaemonStatus {
        let device_config = match DeviceConfigManager::new().load_config() {
            Ok(list) => list,
            Err(_) => vec![],
        };

        let services = SERVICE_MANAGER.collect_status();

        OODDaemonStatus {
            last_update_time: bucky_time_now(),
            device_config,
            services,
        }
    }
}