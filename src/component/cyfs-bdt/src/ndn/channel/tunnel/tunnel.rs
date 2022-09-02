use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::{DynamicTunnel, TunnelState}
};
use super::super::{
    protocol::*, 
    channel::Channel
};
use super::{
    udp::UdpTunnel, 
    tcp::TcpTunnel
};


pub trait ChannelTunnel: std::fmt::Display + Send + Sync {
    fn clone_as_tunnel(&self) -> DynamicChannelTunnel;
    fn state(&self) -> TunnelState; 
    fn raw_ptr_eq(&self, tunnel: &DynamicTunnel) -> bool;
    fn active_timestamp(&self) -> Timestamp;
    fn start_at(&self) -> Timestamp;

    fn on_resent_interest(&self, interest: &Interest) -> BuckyResult<()>;
    fn send_piece_control(&self, control: PieceControl);
    fn on_piece_data(&self, piece: &PieceData) -> BuckyResult<()>;
    fn on_resp_estimate(&self, est: &ChannelEstimate) -> BuckyResult<()>;
    fn on_piece_control(&self, ctrl: &mut PieceControl) -> BuckyResult<()>;

    fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()>;
}

pub type DynamicChannelTunnel = Box<dyn ChannelTunnel>;

pub(in super::super) fn new_channel_tunnel(channel: Channel, raw_tunnel: DynamicTunnel) -> BuckyResult<DynamicChannelTunnel> {
    if let TunnelState::Active(active_timestamp) = raw_tunnel.as_ref().state() {
        if raw_tunnel.as_ref().local().is_udp() {
            Ok(UdpTunnel::new(channel, raw_tunnel.clone_as_tunnel(), active_timestamp).clone_as_tunnel())
        } else if raw_tunnel.as_ref().local().is_tcp() {
            Ok(TcpTunnel::new(channel, raw_tunnel.clone_as_tunnel(), active_timestamp).clone_as_tunnel())
        } else {
            unreachable!()
        }
    } else {
        Err(BuckyError::new(BuckyErrorCode::ErrorState,"tunnel's dead"))
    }
}




