use cyfs_base::{BuckyResult};
use async_trait::async_trait;
use std::future::Future;

#[async_trait]
pub trait ChainNetworkEventEndpoint: 'static + Send + Sync {
    async fn call(&self, data: Vec<u8>) -> BuckyResult<Vec<u8>>;
}

#[async_trait]
impl<F, Fut> ChainNetworkEventEndpoint for F
    where
        F: Send + Sync + 'static + Fn(Vec<u8>) -> Fut,
        Fut: Send + 'static + Future<Output = BuckyResult<Vec<u8>>>,
{
    async fn call(&self, data: Vec<u8>) -> BuckyResult<Vec<u8>> {
        let fut = (self)(data);
        fut.await
    }
}

#[async_trait]
pub trait ChainNetwork: Sync + Send {
    async fn broadcast(&self, obj: Vec<u8>) -> BuckyResult<()>;
    async fn request(&self, param: Vec<u8>, to: Option<String>) -> BuckyResult<Vec<u8>>;
    async fn start(&self, ep: impl ChainNetworkEventEndpoint) -> BuckyResult<()>;
    async fn stop(&self) -> BuckyResult<()>;
    async fn has_connected(&self) -> BuckyResult<bool>;
    async fn local_addr(&self) -> BuckyResult<String>;
    async fn is_local_addr(&self, node: &str) -> BuckyResult<bool>;
    fn get_node_list(&self) -> BuckyResult<Vec<(String, String)>>;
    fn is_node_exist(&self, node: &str) -> BuckyResult<bool>;
    async fn add_node(&self, node_id: &str, node: &str) -> BuckyResult<()>;
    fn get_node(&self, node_id: &str) -> Option<String>;
}
