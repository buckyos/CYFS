mod admin;
mod crypto;
mod crypto_api;
mod interface;
mod meta;
mod name;
//mod default_app;
mod router_handler;
mod stack;
mod storage;
mod sync;
mod trans;
mod trans_api;
mod util;
mod util_api;
mod zone;
mod acl;
mod app;
mod forward;
mod ndn;
pub mod ndn_api;
mod non;
mod non_api;
mod resolver;
mod events;
mod root_state;
mod root_state_api;
mod config;
mod front;
mod rmeta_api;
mod rmeta;

pub use stack::*;
pub use storage::*;
pub use acl::*;
pub use interface::*;
pub use zone::*;

#[macro_use]
extern crate log;

static VERSION: once_cell::sync::OnceCell<&'static str> = once_cell::sync::OnceCell::new();

fn version() -> &'static str {
    VERSION.get().unwrap_or(&"version not inited")
}

pub fn set_version(version: &'static str) {
    let _ = VERSION.set(version);
}

#[cfg(test)]
mod tests {
    #[test]
    fn main() {
    }
}
