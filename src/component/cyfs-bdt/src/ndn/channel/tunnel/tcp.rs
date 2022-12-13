use std::{
    ops::Range
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::{tcp::Tunnel as RawTunnel, Tunnel, DynamicTunnel, TunnelState}, 
    interface
};
use super::super::super::{
    types::*, 
    chunk::ChunkEncoder
};
use super::super::{
    protocol::v0::*, 
};
use super::{
    tunnel::*
};

struct TunnelImpl {
    start_at: Timestamp, 
    active_timestamp: Timestamp, 
    raw_tunnel: RawTunnel, 
    uploaders: Uploaders
}

#[derive(Clone)]
pub struct TcpTunnel(Arc<TunnelImpl>);

impl std::fmt::Display for TcpTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{tunnel:{}}}", self.0.raw_tunnel)
    }
}

impl TcpTunnel {
    pub fn new(
        raw_tunnel: RawTunnel, 
        active_timestamp: Timestamp
    ) -> Self {
        Self(Arc::new(TunnelImpl {
            active_timestamp, 
            start_at: bucky_time_now(), 
            raw_tunnel, 
            uploaders: Uploaders::new()
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

    fn on_piece_data(&self, _piece: &PieceData) -> BuckyResult<()> {
        Ok(())
    }

    fn on_resp_estimate(&self, _est: &ChannelEstimate) -> BuckyResult<()> {
        unreachable!()
    }

    fn on_time_escape(&self, _now: Timestamp) -> BuckyResult<()> {
        while !self.0.raw_tunnel.is_data_piece_full()? {
            let mut piece_buf = [0u8; interface::udp::MTU];
            let piece_len = self.uploaders().next_piece(&mut piece_buf[u16::raw_bytes().unwrap()..]);
            if piece_len > 0 {
                let _ = (piece_len as u16).raw_encode(&mut piece_buf, &None).unwrap();
                let _ = self.0.raw_tunnel.send_data_piece(&mut piece_buf)?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn uploaders(&self) -> &Uploaders {
        &self.0.uploaders
    }

    fn download_state(&self) -> Box<dyn TunnelDownloadState> {
        Box::new(TcpDownloadState {})
    }

    fn upload_state(&self, encoder: Box<dyn ChunkEncoder>) -> Box<dyn ChunkEncoder> {
        WrapEncoder {origin: encoder}.clone_as_encoder()
    }
}

struct WrapEncoder {
    origin: Box<dyn ChunkEncoder>
}

impl ChunkEncoder for WrapEncoder {
    fn clone_as_encoder(&self) -> Box<dyn ChunkEncoder> {
        Box::new(Self {origin: self.origin.clone_as_encoder()})
    }

    fn chunk(&self) -> &ChunkId {
        self.origin.chunk()
    }

    fn desc(&self) -> &ChunkEncodeDesc {
        self.origin.desc()
    }

    fn next_piece(
        &self, 
        session_id: &TempSeq, 
        buf: &mut [u8]
    ) -> BuckyResult<usize> {
        self.origin.next_piece(session_id, buf)
    }

    fn reset(&self) -> bool {
        false
    }   

    fn merge(
        &self, 
        _max_index: u32, 
        _lost_index: Vec<Range<u32>>
    ) -> bool {
        false 
    }
}

struct TcpDownloadState {
}

impl TunnelDownloadState for TcpDownloadState {
    fn on_piece_data(&mut self) {
        
    }

    fn on_resp_interest(&mut self) {
        
    }

    fn on_time_escape(&mut self, _now: Timestamp) -> bool {
        false
    }
}
