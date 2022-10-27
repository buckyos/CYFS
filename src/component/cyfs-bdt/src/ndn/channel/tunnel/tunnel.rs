use std::{
    sync::RwLock
};
use cyfs_base::*;
use crate::{
    types::*, 
    tunnel::{DynamicTunnel, TunnelState}
};
use super::super::super::{
    types::*
};

use super::super::{
    protocol::v0::*, 
    channel::Channel, 
    upload::*
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
    fn uploaders(&self) -> &Uploaders;
}

pub type DynamicChannelTunnel = Box<dyn ChannelTunnel>;

pub(in super::super) fn new_channel_tunnel(channel: &Channel, raw_tunnel: DynamicTunnel) -> BuckyResult<DynamicChannelTunnel> {
    if let TunnelState::Active(active_timestamp) = raw_tunnel.as_ref().state() {
        if raw_tunnel.as_ref().local().is_udp() {
            Ok(UdpTunnel::new(channel.config().udp.clone(), raw_tunnel.clone_as_tunnel(), active_timestamp).clone_as_tunnel())
        } else if raw_tunnel.as_ref().local().is_tcp() {
            Ok(TcpTunnel::new(raw_tunnel.clone_as_tunnel(), active_timestamp).clone_as_tunnel())
        } else {
            unreachable!()
        }
    } else {
        Err(BuckyError::new(BuckyErrorCode::ErrorState,"tunnel's dead"))
    }
}


struct UploadersImpl {
    sessions: Vec<UploadSession>,
    speed_counter: SpeedCounter,
    piece_seq: u64
}


pub(in super::super) struct Uploaders(RwLock<UploadersImpl>);


impl Uploaders {
    pub fn new() -> Self {
        Self(RwLock::new(UploadersImpl {
            sessions: vec![], 
            speed_counter: SpeedCounter::new(0), 
            piece_seq: 0, 
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.0.read().unwrap().sessions.is_empty()
    }

    pub fn find(&self, session_id: &TempSeq) -> Option<UploadSession> {
        self.0.read().unwrap().sessions.iter().find(|session| session.session_id().eq(session_id)).cloned()
    }

    pub fn add(&self, session: UploadSession) {
        let sessions = &mut self.0.write().unwrap().sessions;
        if sessions.iter().find(|s| session.session_id().eq(s.session_id())).is_none() {
            // info!("{} add upload session {}", session.channel(), session);
            sessions.push(session);
        }
    }

    // pub fn remove(&self, session_id: &TempSeq) -> Option<UploadSession> {
    //     let mut sessions = self.sessions.write().unwrap();
    //     if let Some((i, _)) = sessions.uploading.iter().enumerate().find(|(_, session)| session_id.eq(session.session_id())) {
    //         let session = sessions.uploading.remove(i);
    //         info!("{} remove {}", session.channel(), session);
    //         Some(session)
    //     } else {
    //         None
    //     }
    // }

    // fn cancel_by_error(&self, err: BuckyError) {
    //     let uploading = self.sessions.read().unwrap().uploading.clone();
    //     for session in &uploading {
    //         session.cancel_by_error(BuckyError::new(err.code(), err.msg().to_string()));
    //     }
    //     let mut sessions = self.sessions.write().unwrap();
    //     for session in uploading {
    //         if let Some((i, _)) = sessions.uploading.iter().enumerate().find(|(_, s)| session.session_id().eq(s.session_id())) {
    //             let _ = sessions.uploading.remove(i);
    //             sessions.canceled.push_back(session);
    //         }
    //     }
    // }

    pub fn next_piece(&self, buf: &mut [u8]) -> usize {
        let mut try_count = 0;
        let len = loop {
            let ret = {
                let mut state = self.0.write().unwrap();
                if state.sessions.len() > 0 {
                    let seq = state.piece_seq;
                    state.piece_seq += 1;
                    let index = (seq % (state.sessions.len() as u64)) as usize;
                    Some((state.sessions.get(index).unwrap().clone(), state.sessions.len()))
                } else {
                    None
                }
            };
            
            if let Some((session, session_count)) = ret {
                match session.next_piece(buf) {
                    Ok(len) => {
                        if len > 0 {
                            break len;
                        } else {
                            try_count += 1;
                            if try_count >= session_count {
                                break 0;
                            }
                        }
                    },
                    Err(err) => {
                        // debug!("{} cancel {} for next piece failed for {}", session.channel(), session, err);
                        {   
                            let mut state = self.0.write().unwrap();
                            if let Some((i, _)) = state.sessions.iter().enumerate().find(|(_, s)| session.session_id().eq(s.session_id())) {
                                let _ = state.sessions.remove(i);
                                // info!("{} remove {}", session.channel(), session);
                            }
                        }
                        try_count += 1;
                        if try_count >= session_count {
                            break 0;
                        }
                    }
                }
            } else {
                break 0;
            }
        };

        if len > 0 {
            self.0.write().unwrap().speed_counter.on_recv(len);
        }

        len
    }

    // fn on_time_escape(&self, now: Timestamp) {
    //     let mut sessions = self.sessions.write().unwrap();

    //     let mut uploading = vec![];
    //     std::mem::swap(&mut sessions.uploading, &mut uploading);
        
    //     for session in uploading {
    //         if let Some(state) = session.on_time_escape(now) {
    //             match state {
    //                 UploadTaskState::Finished => {
    //                     sessions.canceled.push_back(session);
    //                 },
    //                 UploadTaskState::Error(_) => {
    //                     sessions.canceled.push_back(session);
    //                 }, 
    //                 _ => {
    //                     sessions.uploading.push(session);
    //                 }
    //             }
    //         } else {
    //             info!("{} remove session {}", session.channel(), session);
    //         }
    //     }

    //     let mut canceled = LinkedList::new();
    //     std::mem::swap(&mut sessions.canceled, &mut canceled);
    //     for session in canceled {
    //         if let Some(_) = session.on_time_escape(now) {
    //             sessions.canceled.push_back(session);
    //         } else {
    //             info!("{} remove session {}", session.channel(), session);
    //         }
    //     }
    // }

    // fn session_count(&self) -> u32 {
    //     self.0.read().unwrap().uploading.len() as u32 
    // }

    fn calc_speed(&self, when: Timestamp) -> u32 {
        let mut state = self.0.write().unwrap();
        state.speed_counter.update(when)
    }
}



