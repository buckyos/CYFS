mod auth;
mod http_bdt_listener;
mod http_listener;
mod http_server;
mod http_tcp_listener;
mod http_ws_listener;
mod listener_manager;
mod sync_interface;
mod ws_interface;
mod browser_server;

pub(crate) use auth::InterfaceAuth;
use http_bdt_listener::*;
use http_listener::*;
pub use http_server::*;
use http_tcp_listener::*;
pub(crate) use listener_manager::*;
pub(crate) use sync_interface::*;
use ws_interface::*;
pub(crate) use browser_server::BrowserSanboxMode;

use cyfs_base::BuckyResult;
use cyfs_lib::RequestProtocol;

use async_std::net::SocketAddr;
use async_trait::async_trait;

#[async_trait]
trait ObjectListener: Send + Sync {
    fn get_protocol(&self) -> RequestProtocol;

    fn get_addr(&self) -> SocketAddr;

    async fn start(&self) -> BuckyResult<()>;

    async fn stop(&self) -> BuckyResult<()>;

    async fn restart(&self) -> BuckyResult<()>;
}
