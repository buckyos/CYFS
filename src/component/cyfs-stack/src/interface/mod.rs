mod http_bdt_listener;
mod http_listener;
mod http_server;
mod http_tcp_listener;
mod http_ws_listener;
mod listener_manager;
mod sync_interface;
mod translator;
mod ws_interface;

use http_bdt_listener::*;
use http_listener::*;
pub use http_server::*;
use http_tcp_listener::*;
pub(crate) use listener_manager::*;
pub(crate) use sync_interface::*;
use ws_interface::*;

use cyfs_base::BuckyResult;
use cyfs_lib::NONProtocol;

use async_std::net::SocketAddr;
use async_trait::async_trait;

#[async_trait]
trait ObjectListener: Send + Sync {
    fn get_protocol(&self) -> NONProtocol;

    fn get_addr(&self) -> SocketAddr;

    async fn start(&self) -> BuckyResult<()>;

    async fn stop(&self) -> BuckyResult<()>;

    async fn restart(&self) -> BuckyResult<()>;
}
