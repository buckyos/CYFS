use cyfs_base::*;
use super::proxy::ProxyDeviceStub;

#[async_trait::async_trait]
pub trait ProxyServiceEvents: Send + Sync {
    async fn pre_create_tunnel(&self, mix_key: &AesKey, device_pair: &(ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()>;
}