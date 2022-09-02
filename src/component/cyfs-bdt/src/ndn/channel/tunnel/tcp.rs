use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::{tcp::Tunnel as RawTunnel, Tunnel, DynamicTunnel, TunnelState}, 
    interface
};
use super::super::{
    protocol::*, 
    channel::Channel
};
use super::{
    tunnel::{ChannelTunnel, DynamicChannelTunnel}
};

struct TunnelImpl {
    channel: Channel, 
    start_at: Timestamp, 
    active_timestamp: Timestamp, 
    raw_tunnel: RawTunnel
}

#[derive(Clone)]
pub struct TcpTunnel(Arc<TunnelImpl>);

impl std::fmt::Display for TcpTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {{tunnel:{}}}", self.0.channel, self.0.raw_tunnel)
    }
}

impl TcpTunnel {
    pub fn new(channel: Channel, raw_tunnel: RawTunnel, active_timestamp: Timestamp) -> Self {
        Self(Arc::new(TunnelImpl {
            channel, 
            active_timestamp, 
            start_at: bucky_time_now(), 
            raw_tunnel
        }))
    }
}

impl ChannelTunnel for TcpTunnel {
    fn clone_as_tunnel(&self) -> DynamicChannelTunnel {
        Box::new(self.clone())
    }

    fn raw_ptr_eq(&self, tunnel: &DynamicTunnel) -> bool {
        self.0.raw_tunnel.ptr_eq(tunnel)
    }

    fn state(&self) -> TunnelState {
        self.0.raw_tunnel.state()
    } 

    fn start_at(&self) -> Timestamp {
        self.0.start_at
    }

    fn active_timestamp(&self) -> Timestamp {
        self.0.active_timestamp
    }

    fn send_piece_control(&self, control: PieceControl) {
        if control.command != PieceControlCommand::Continue && ctrl.max_index.is_some() {
            debug!("{} will send piece control {:?}", self, control);
            let _ = control.split_send(&DynamicTunnel::new(self.0.raw_tunnel.clone()));
        }
    }

    fn on_piece_data(&self, _piece: &PieceData) -> BuckyResult<()> {
        Ok(())
    }

    fn on_resp_estimate(&self, _est: &ChannelEstimate) -> BuckyResult<()> {
        unreachable!()
    }

    fn on_piece_control(&self, ctrl: &mut PieceControl) -> BuckyResult<()> {
        if PieceControlCommand::Continue == ctrl.command && ctrl.max_index.is_some() { 
            info!("{} will discard send buffer to resend", self);
            let _ = self.0.raw_tunnel.discard_data_piece();
        }
        Ok(())
    }

    fn on_time_escape(&self, _now: Timestamp) -> BuckyResult<()> {
        while !self.0.raw_tunnel.is_data_piece_full()? {
            let mut piece_buf = [0u8; interface::udp::MTU];
            let piece_len = self.0.channel.next_piece(&mut piece_buf[u16::raw_bytes().unwrap()..]);
            if piece_len > 0 {
                let _ = (piece_len as u16).raw_encode(&mut piece_buf, &None).unwrap();
                let _ = self.0.raw_tunnel.send_data_piece(&mut piece_buf)?;
            } else {
                break;
            }
        }
        Ok(())
    }
}