mod bdt_loader;
mod cyfs_loader;
mod cyfs_loader_config;
mod cyfs_stack_loader;
mod known_objects;
mod listener_util;
mod local_device_manager;
mod random_port;
mod stack_info;
mod stack_manager;
mod var_manager;

pub use crate::cyfs_loader::*;
pub use cyfs_loader_config::*;
pub use known_objects::*;
pub use listener_util::*;
pub use local_device_manager::*;
pub use stack_manager::*;
pub use var_manager::*;
pub use var_manager::*;

pub use cyfs_stack::*;

#[macro_use]
extern crate log;

pub fn set_version(ver: &'static str) {
    cyfs_stack::set_version(ver);
}