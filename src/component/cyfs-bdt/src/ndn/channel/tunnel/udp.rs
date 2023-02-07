use log::*;
use std::{
    collections::{LinkedList},
    time::{Duration},
    cell::RefCell, 
    sync::Mutex
};
// use cyfs_debug::Mutex;
use async_std::{
    sync::Arc, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    interface::udp::MTU, 
    tunnel::{udp::Tunnel as RawTunnel, Tunnel, DynamicTunnel, TunnelState}, 
    cc::{self, CongestionControl},
};
use super::super::super::{
    chunk::ChunkEncoder
};
use super::super::{
    protocol::v0::*, 
    channel::{self}
};
use super::{
    tunnel::*
};


#[derive(Clone)]
pub struct Config {  
    pub no_resp_loss_count: u32, 
    pub break_loss_count: u32, 
    pub cc: cc::Config, 
}


struct EstimateStub {
    pub seq: TempSeq, 
    pub send_time: Timestamp, 
    pub sent: usize /*距离上一个est之间发出去多少*/
}


struct CcImpl {
    est_stubs: LinkedList<EstimateStub>,
    est_seq: TempSeqGenerator,  
    on_air: usize,
    cc: CongestionControl, 
    no_resp_counter: u32,
    break_counter: u32,  
}

impl CcImpl {
    fn new(config: &cc::Config, init_seq: TempSeq) -> Self {
        Self {
            est_stubs: LinkedList::new(), 
            est_seq: TempSeqGenerator::from(init_seq), 
            on_air: 0, 
            cc: CongestionControl::new(PieceData::max_payload(), config), 
            no_resp_counter: 0, 
            break_counter: 0
        }
    }
}


struct RespEstimateStub {
    seq: TempSeq,
    recved: u64,
}

struct TunnelImpl {
    config: channel::Config, 
    raw_tunnel: RawTunnel, 
    start_at: Timestamp, 
    active_timestamp: Timestamp, 
    cc: Mutex<CcImpl>, 
    resp_estimate: Mutex<RespEstimateStub>, 
    uploaders: Uploaders
}

#[derive(Clone)]
pub struct UdpTunnel(Arc<TunnelImpl>);

impl std::fmt::Display for UdpTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{tunnel:{}}}", self.0.raw_tunnel)
    }
}

impl UdpTunnel {
    pub fn new(
        config: channel::Config, 
        raw_tunnel: RawTunnel, 
        active_timestamp: Timestamp) -> Self {
        let cc = CcImpl::new(&config.udp.cc, raw_tunnel.owner().map(|t| t.generate_sequence()).unwrap_or_default());
        Self(Arc::new(TunnelImpl {
            config, 
            raw_tunnel, 
            start_at: bucky_time_now(), 
            active_timestamp, 
            cc: Mutex::new(cc), 
            resp_estimate: Mutex::new(RespEstimateStub {
                seq: TempSeq::default(), 
                recved: 0
            }), 
            uploaders: Uploaders::new()
        }))
    }

    fn config(&self) -> &channel::Config {
        &self.0.config
    }

    fn send_pieces(&self, piece_count: usize) {
        if piece_count == 0 {
            return;
        }
        // trace!("{} schedule send pieces count {}", self, piece_count);
        struct BufferIndex {
            index: usize, 
            len: usize
        }
        PIECE_BUFFER_A.with(|thread_piece_buf_a| {
            PIECE_BUFFER_B.with(|thread_piece_buf_b| {
                let buffers = [
                    &mut thread_piece_buf_a.borrow_mut()[..], 
                    &mut thread_piece_buf_b.borrow_mut()[..]
                ];
                let mut pre_buf_index: Option<BufferIndex> = None;
                let mut sent = 0;
                let tunnel = &self.0.raw_tunnel;
                for _ in 0..piece_count {
                    let mut buf_index = if let Some(bi) = &pre_buf_index {
                        if bi.index == 0 {
                            BufferIndex {index: 1, len: 0}
                        } else if bi.index == 1 {
                            BufferIndex {index: 0, len: 0}
                        } else {
                            unreachable!()
                        }
                    } else {
                        BufferIndex {index: 0, len: 0}
                    };
                    let piece_len = self.uploaders().next_piece(&mut buffers[buf_index.index][tunnel.raw_data_header_len()..]);
                    if piece_len > 0 {
                        sent += 1;
                        buf_index.len = piece_len;
                        if pre_buf_index.is_some() {
                            std::mem::swap(pre_buf_index.as_mut().unwrap(), &mut buf_index);
                            let _ = tunnel.send_raw_data(&mut buffers[buf_index.index][..buf_index.len + tunnel.raw_data_header_len()]);
                        } else {
                            pre_buf_index = Some(buf_index);
                        }
                    } else {
                        break;
                    }
                }

                if let Some(buf_index) = pre_buf_index {
                    let est_seq = {
                        let mut cc = self.0.cc.lock().unwrap();
                        cc.on_air += sent;
                        let seq = cc.est_seq.generate();
                        cc.est_stubs.push_back(EstimateStub {
                            seq,
                            send_time: bucky_time_now(), 
                            sent
                        });
                        seq
                    };
                    debug!("{} send estimate sequence:{:?} sent:{}", self, est_seq, sent);
                    PieceData::reset_estimate(&mut buffers[buf_index.index][tunnel.raw_data_header_len()..], est_seq);
                    let _ = tunnel.send_raw_data(&mut buffers[buf_index.index][..buf_index.len + tunnel.raw_data_header_len()]);
                }
            })
        });      
    }
}


thread_local! {
    static PIECE_BUFFER_A: RefCell<Vec<u8>> = RefCell::new(vec![0u8; MTU]);
    static PIECE_BUFFER_B: RefCell<Vec<u8>> = RefCell::new(vec![0u8; MTU]);
}

impl ChannelTunnel for UdpTunnel {
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

    fn on_piece_data(&self, piece: &PieceData) -> BuckyResult<()> {
        trace!("{} got piece data est_seq:{:?} chunk:{} desc:{:?} data:{}", self, piece.est_seq, piece.chunk, piece.desc, piece.data.len());
        if let Some(est_seq) = piece.est_seq {
            if let Some(resp) = {
                debug!("{} got estimate seqenuce:{:?}", self, est_seq);

                let mut est_stub = self.0.resp_estimate.lock().unwrap();
                est_stub.recved += 1;
                if est_stub.seq < est_seq {
                    est_stub.seq = est_seq;
                } 
                let resp = ChannelEstimate {
                    sequence: est_seq, 
                    recved: est_stub.recved,
                };
                Some(resp)
            } {
                let tunnel = &self.0.raw_tunnel;
                let mut buffer = vec![0u8; tunnel.raw_data_header_len() + resp.raw_measure(&None).unwrap()];
                let _ = resp.raw_encode(&mut buffer[tunnel.raw_data_header_len()..], &None).unwrap();
                debug!(
                    "{} will send resp estimate with {{sequence:{:?}}}",
                    self,
                    est_seq
                );
                let _ = tunnel.send_raw_data(&mut buffer[..]);
            }
        } else {
            let mut est_stub = self.0.resp_estimate.lock().unwrap();
            est_stub.recved += 1;
        }
        Ok(())
    }

    fn on_resp_estimate(&self, est: &ChannelEstimate) -> BuckyResult<()> {
        debug!("{} recv RespEstimate with sequence {:?}", self, est.sequence);
        // 对 estimate rtt的回复  
        let mut cc = self.0.cc.lock().unwrap();
            
        let mut est_index = None;

        for (index, stub) in cc.est_stubs.iter().rev().enumerate() {
            if stub.seq == est.sequence {
                let rtt = Duration::from_micros(bucky_time_now() - stub.send_time);
                let delay = rtt / 2;
                
                cc.cc.on_estimate(rtt, delay);
                debug!("{} estimate rtt:{:?} delay:{:?} rto:{:?}", self, rtt, delay, cc.cc.rto());

                est_index = Some(cc.est_stubs.len() - 1 - index);

                break;
            } 
        }
    
        if let Some(est_index) = est_index {
            let mut resp_count = 0;

            let est_stubs = cc.est_stubs.split_off(est_index + 1);
            for stub in &cc.est_stubs {
                resp_count += stub.sent;
            }
            cc.est_stubs = est_stubs;

            cc.on_air -= std::cmp::min(cc.on_air, resp_count);
            
            let on_air = cc.on_air;
            debug!("{} cc on ack on_air:{}, ack:{}", self, on_air, resp_count);
            cc.no_resp_counter = 0;
            cc.break_counter = 0;
            cc.cc.on_ack(
                (on_air * PieceData::max_payload()) as u64, 
                (resp_count * PieceData::max_payload()) as u64, 
            	None,
            	bucky_time_now());
        }
        
        Ok(())
    }



    fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()> {
        if TunnelState::Dead == self.0.raw_tunnel.state() {
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, "tunnel's dead"));
        }
        let send_count = {
            let mut cc = self.0.cc.lock().unwrap();
            cc.cc.on_time_escape(now);

            let mut loss_from_index = None;
            let mut loss_count = 0;
            for (index, stub) in cc.est_stubs.iter().enumerate() {
                if now > stub.send_time && Duration::from_micros(now - stub.send_time) > cc.cc.rto() {
                    loss_count += stub.sent;
                    loss_from_index = Some(index);
                } else {
                    break;
                }
            }

            if let Some(index) = loss_from_index {
                cc.no_resp_counter += 1;
                cc.break_counter += 1;
                if cc.break_counter > self.config().udp.break_loss_count {
                    cc.no_resp_counter = 0;
                    cc.break_counter = 0;
                    cc.est_stubs = LinkedList::new();
                    cc.on_air = 0;
                    Err(BuckyError::new(BuckyErrorCode::Timeout, "udp outcome break"))
                } else {
                    cc.est_stubs = cc.est_stubs.split_off(index + 1);
                    cc.on_air -= loss_count;
                    if cc.no_resp_counter > self.config().udp.no_resp_loss_count  {
                        debug!("{} outcome on no resp {} rto {:?} ", self, loss_count, cc.cc.rto());
                        cc.no_resp_counter = 0;
                        cc.cc.on_no_resp((loss_count * PieceData::max_payload()) as u64);
                    } else {
                        debug!("{} outcome on loss count {} rto {:?} ", self, loss_count, cc.cc.rto());
                        cc.cc.on_loss((loss_count * PieceData::max_payload()) as u64);
                    }

                    let cwnd = (cc.cc.cwnd() / PieceData::max_payload() as u64) as usize;
                    if cwnd > cc.on_air {
                        Ok(cwnd - cc.on_air)
                    } else {
                        Ok(0)
                    }
                } 
            } else {
                let cwnd = (cc.cc.cwnd() / PieceData::max_payload() as u64) as usize;
                if cwnd > cc.on_air {
                    Ok(cwnd - cc.on_air)
                } else {
                    Ok(0)
                }
            }
        }.map_err(|err| {
            self.0.raw_tunnel.mark_dead(TunnelState::Active(self.0.active_timestamp));
            err
        })?;
        self.send_pieces(send_count);
        Ok(())
    }

    fn uploaders(&self) -> &Uploaders {
        &self.0.uploaders
    }

    fn download_state(&self) -> Box<dyn TunnelDownloadState> {
        Box::new(UdpDownloadState {
            config: self.config().clone(), 
            last_pushed: None, 
            next_send_time: None
        })
    }

    fn upload_state(&self, encoder: Box<dyn ChunkEncoder>) -> Box<dyn ChunkEncoder> {
        encoder
    }
}


#[derive(Clone, Copy)]
enum LastPushed {
    PieceData(Timestamp), 
    RespInterest(Timestamp)
}

impl LastPushed {
    fn time(&self) -> Timestamp {
        match self {
            Self::PieceData(at) => *at, 
            Self::RespInterest(at) => *at
        }
    }
}

struct UdpDownloadState {
    config: channel::Config, 
    last_pushed: Option<LastPushed>,  
    next_send_time: Option<Timestamp>
}

impl TunnelDownloadState for UdpDownloadState {
    fn on_piece_data(&mut self) {
        let now = bucky_time_now();
        if let Some(last_pushed) = self.last_pushed {
            if now > last_pushed.time() {
                match last_pushed {
                    LastPushed::PieceData(at) => {
                        let interval = u64::min(2 * (now - at), self.config.udp.cc.min_rto.as_micros() as u64);
                        let interval = u64::min(self.config.block_interval.as_micros() as u64, interval);
                        self.last_pushed = Some(LastPushed::PieceData(now));
                        self.next_send_time = Some(now + interval);
                    },
                    LastPushed::RespInterest(_) => {
                        self.last_pushed = Some(LastPushed::PieceData(now));
                        self.next_send_time = Some(now + self.config.block_interval.as_micros() as u64);
                    }
                }
            }
        } else {
            self.last_pushed = Some(LastPushed::PieceData(now));
            self.next_send_time = Some(now + self.config.block_interval.as_micros() as u64);
        }
    }

    fn on_resp_interest(&mut self) {
        let now = bucky_time_now();
        if let Some(last_pushed) = self.last_pushed {
            if now > last_pushed.time() {
                match last_pushed {
                    LastPushed::PieceData(_) => {
                        self.last_pushed = Some(LastPushed::RespInterest(now));
                        self.next_send_time = Some(now + self.config.block_interval.as_micros() as u64);
                    },
                    LastPushed::RespInterest(at) => {
                        let interval = 2 * (now - at);
                        self.last_pushed = Some(LastPushed::RespInterest(now));
                        self.next_send_time = Some(now + interval);
                    }
                }
            }
        } else {
            self.last_pushed = Some(LastPushed::RespInterest(now));
            self.next_send_time = Some(now + self.config.block_interval.as_micros() as u64);
        }
    }


    fn on_time_escape(&mut self, now: Timestamp) -> bool {
        if let Some(next_send_time) = self.next_send_time {
            if now > next_send_time {
                let interval = next_send_time - self.last_pushed.clone().unwrap().time();
                self.next_send_time = Some(next_send_time + interval);
                true
            } else {
                false
            }
        } else {
            false
        }
    }
}
