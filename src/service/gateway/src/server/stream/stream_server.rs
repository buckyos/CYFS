use async_trait::async_trait;

use cyfs_base::BuckyError;

pub enum StreamServerProtocol {
    TCP,
    UDP,
}

#[async_trait]
pub trait StreamServer: Send + Sync {
    fn load(&mut self, server_node: &toml::value::Table) -> Result<(), BuckyError>;
    fn start(&self) -> Result<(), BuckyError>;
    fn stop(&self);
}
