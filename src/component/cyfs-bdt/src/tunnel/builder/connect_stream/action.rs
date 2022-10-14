use async_trait::{async_trait};
use cyfs_base::*;
use crate::{
    stream::{StreamProviderSelector}
};
use super::super::{action::*};

pub enum ConnectStreamState {
    Connecting1, 
    PreEstablish,
    Connecting2,  
    Establish, 
    Closed
}

pub type DynConnectStreamAction = Box<dyn ConnectStreamAction>;

#[async_trait]
pub trait ConnectStreamAction: BuildTunnelAction {
    fn clone_as_connect_stream_action(&self) -> DynConnectStreamAction;
    fn as_any(&self) -> &dyn std::any::Any;
    fn state(&self) -> ConnectStreamState;
    async fn wait_pre_establish(&self) -> ConnectStreamState;
    async fn continue_connect(&self) -> BuckyResult<StreamProviderSelector>;
}