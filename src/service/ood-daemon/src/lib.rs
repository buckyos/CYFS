mod config;
mod package;
mod repo;
mod config_repo;
mod service;

pub use config::{DEVICE_CONFIG_MANAGER, init_system_config, get_system_config, ServiceState};
pub use service::{ServiceMode, SERVICE_MANAGER};
pub use repo::{REPO_MANAGER, RepoManager};

#[macro_use]
extern crate log;