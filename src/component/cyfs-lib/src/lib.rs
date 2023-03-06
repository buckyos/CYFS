mod acl;
mod admin;
mod base;
mod crypto;
mod default_app;
mod events;
mod ndn;
mod non;
mod prelude;
mod rmeta;
mod root_state;
mod router_handler;
mod stack;
mod storage;
mod sync;
mod trans;
mod util;
mod ws;
mod zone;
mod requestor;

pub use crate::util::*;
pub use acl::*;
pub use admin::*;
pub use base::*;
pub use crypto::*;
pub use default_app::*;
pub use events::*;
pub use ndn::*;
pub use non::*;
pub use prelude::*;
pub use rmeta::*;
pub use root_state::*;
pub use router_handler::*;
pub use stack::*;
pub use storage::*;
pub use sync::*;
pub use trans::*;
pub use ws::*;
pub use zone::*;
pub use requestor::*;

// 重新导出cache相关接口，由于bdt层的依赖关系，只能放在util工程
pub use cyfs_util::cache::*;

#[macro_use]
extern crate log;

pub fn register_core_objects_format() {
    use crate::admin::*;
    use cyfs_base::*;

    FORMAT_FACTORY.register(cyfs_core::CoreObjectType::Admin, format_json::<AdminObject>);
}

cyfs_base::declare_module_perf_isolate!("cyfs-lib");

#[cfg(test)]
mod tests {
    #[test]
    fn main() {
    }
}
