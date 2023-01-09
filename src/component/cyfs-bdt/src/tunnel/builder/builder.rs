use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::{*, v0::*}, 
};

pub enum TunnelBuilderState {
    Connecting, 
    Establish, 
    Closed
}

#[async_trait]
pub trait TunnelBuilder: Send + Sync + OnPackage<AckProxy, &'static DeviceId> {
    fn sequence(&self) -> TempSeq;
    fn state(&self) -> TunnelBuilderState;
    async fn wait_establish(&self) -> Result<(), BuckyError>;
}