use crate::config::ServiceState;

use serde::Serialize;


#[derive(Serialize)]
pub enum OODServicePackageStatus {
    Downloading,
    Ready,
}

#[derive(Serialize)]
pub enum OODServiceProcessStatus {
    Stopped,
    Running,
}

#[derive(Serialize)]
pub struct OODServiceStatusItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enable: bool,
    pub target_state: ServiceState,
    pub package_state: OODServicePackageStatus,
    pub service_state: OODServiceProcessStatus,
}

#[derive(Serialize)]
pub struct OODDaemonStatus {
    services: Vec<OODServiceStatusItem>,
}