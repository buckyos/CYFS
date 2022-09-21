use cyfs_base::*;
use super::proxy::ProxyDeviceStub;

#[async_trait::async_trait]
pub trait ProxyServiceEvents: Send + Sync {
    async fn pre_create_tunnel(&self, key: &KeyMixHash, device_pair: &(ProxyDeviceStub, ProxyDeviceStub), mix_key: &AesKey) -> BuckyResult<()>;
}