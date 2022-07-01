use log::*;
use std::{
    time::Duration, 
    collections::LinkedList, 
};
use cyfs_base::*;
use crate::{
    types::*, 
    protocol::*
};
use super::stream::PackageStream;

#[derive(Debug)]
enum BlockState {
    Wait,
    OnAir(Timestamp),
}

struct Block {
    pub data: Vec<u8>,
    pub start: u64,
    pub state: BlockState,
    pub fin: bool
}

impl Block {
    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn end(&self) -> u64 {
        self.start + self.len() as u64
    }

    pub fn to_session_data(&self, when: Timestamp) -> SessionData {
        let mut package = SessionData::new();
        package.flags_add(SESSIONDATA_FLAG_PAYLOAD);
        package.stream_pos = self.start;
        package.send_time = when;
        package.payload = TailedOwnedData::from(&self.data[..]);
        if self.fin {
            package.flags_add(SESSIONDATA_FLAG_FIN);
        }
        package
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Block [{}..{}) state {:?} fin {}", self.start, self.end(), self.state, self.fin)
    }
}

enum NagleState {
    None, 
    Nagle(Timestamp, usize)
}

struct ValueCache {
    flight: usize, 
    earliest_send_time: Timestamp
}

pub struct SendQueue {
    capacity: usize, 
    start: u64, 
    nagle_buffer: Vec<u8>, 
    nagle_state: NagleState, 
    blocks: LinkedList<Block>, 
    value_cache: ValueCache, 
}


impl SendQueue {
    pub fn new(capacity: usize, start: u64) -> Self {
        Self {
            capacity, 
            start, 
            nagle_buffer: vec![0u8; PackageStream::mss()], 
            nagle_state: NagleState::None, 
            blocks: LinkedList::new(), 
            value_cache: ValueCache {
                flight: 0, 
                earliest_send_time: u64::MAX
            }
        }
    }

    pub fn flight(&self) -> usize {
        self.value_cache.flight
    }

    pub fn used(&self) -> usize {
        (self.block_end() - self.start) as usize + match &self.nagle_state {
            NagleState::Nagle(_, len) => *len, 
            NagleState::None => 0
        }
    }

    pub fn remain(&self) -> usize {
        self.capacity - self.used()
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    fn block_end(&self) -> u64 {
        if let Some(block) = self.blocks.back() {
            block.end()
        } else {
            self.start
        }
    }

    fn alloc_block_from_owned(&mut self, stream: &PackageStream, data: Vec<u8>, fin: bool) {
        let block = Block {
            data, 
            start: self.block_end(), 
            state: BlockState::Wait, 
            fin 
        };
        trace!("{} append send queue {}", stream, block);
        self.blocks.push_back(block);
    }

    fn alloc_block_from_shared(&mut self, stream: &PackageStream, data: &[u8], fin: bool) {
        let data = Vec::from(data);
        self.alloc_block_from_owned(stream, data, fin)
    }

    pub fn alloc_blocks(&mut self, stream: &PackageStream, buf: &[u8]) -> usize {
        if self.remain() == 0 {
            return 0;
        }
        let mut remain = {
            if buf.len() > self.remain() {
                &buf[..self.remain()]
            } else {
                buf
            }
        };
        let writen = remain.len();
        let nagle = match &mut self.nagle_state {
            NagleState::None => {
                if remain.len() < PackageStream::mss() {
                    self.nagle_buffer[..remain.len()].copy_from_slice(remain);
                    self.nagle_state = NagleState::Nagle(bucky_time_now(), remain.len());
                    trace!("{} enter nagle {}", stream, remain.len());
                    Some(())
                } else {
                    None
                }
            }, 
            NagleState::Nagle(_, len) => {
                let total = remain.len() + *len;
                let nagle_len = {
                    if total < PackageStream::mss() {  
                        remain.len()
                    } else {
                        PackageStream::mss() - *len
                    }
                };
                self.nagle_buffer[*len..*len + nagle_len].copy_from_slice(&remain[..nagle_len]);
                *len += nagle_len;

                remain = &remain[nagle_len..];
                
                if total >= PackageStream::mss() {
                    trace!("{} exit send nagle {}", stream, self.nagle_buffer.len());
                    self.alloc_block_from_owned(stream, Vec::from(&self.nagle_buffer[..]), false);
                    self.nagle_state = NagleState::None;
                } else {
                    trace!("{} append send nagle to {}", stream, *len);
                }
                None
            }
        };

        if nagle.is_none() {
            while remain.len() >= PackageStream::mss() {
                self.alloc_block_from_shared(stream, &remain[..PackageStream::mss()], false);
                remain = &remain[PackageStream::mss()..];
            }
            if remain.len() > 0 {
                self.nagle_buffer[..remain.len()].copy_from_slice(remain);
                self.nagle_state = NagleState::Nagle(bucky_time_now(), remain.len());
            }
        }
        writen
    }

    pub fn confirm(&mut self, stream: &PackageStream, ack_pos: u64, fin_ack: bool) -> (usize, bool) {
        let mut newly_acked = 0; 
        let mut fin = false;
        let mut to_remove = None;
        let mut dec_flight = 0;
        for (index, block) in (&self.blocks).iter().enumerate() {
            if block.end() <= ack_pos {
                if block.fin { 
                    if !fin_ack {
                        break;
                    } else {
                        fin = true;
                    }
                }
                newly_acked += block.len();
                to_remove = Some(index);
                if let BlockState::OnAir(_) = &block.state {
                    dec_flight += block.len();
                }
                trace!("{} send queue confirm block [{}..{})", stream, block.start, block.end());
            } else {
                break;
            }
        }

        if let Some(index) = to_remove {
            self.blocks = self.blocks.split_off(index + 1);
        }
        self.value_cache.flight -= dec_flight;
        self.start += newly_acked as u64;
        (newly_acked, fin)
    }

    pub fn check_nagle(&mut self, stream: &PackageStream, now: Timestamp) -> bool {
        match &self.nagle_state {
            NagleState::Nagle(timestamp, len) => {
                if now > *timestamp && Duration::from_micros(now - *timestamp) > stream.config().nagle {
                    trace!("{} exit send nagle {}", stream, *len);
                    let data = Vec::from(&self.nagle_buffer[..*len]);
                    self.alloc_block_from_owned(stream, data, false);
                    self.nagle_state = NagleState::None;
                    true
                } else {
                    false
                }
            }, 
            NagleState::None => {
                false
            }
        }
    }

    pub fn close(&mut self, stream: &PackageStream) {
        if self.blocks.len() == 0 || !self.blocks.back().unwrap().fin {
            match &self.nagle_state {
                NagleState::Nagle(_, len) => {
                    trace!("{} exit send nagle for close {}", stream, *len);
                    let data = Vec::from(&self.nagle_buffer[..*len]);
                    self.alloc_block_from_owned(stream, data, true);
                    self.nagle_state = NagleState::None;
                },  
                NagleState::None => {
                    self.alloc_block_from_owned(stream, vec![0u8;0], true);
                }
            }
        }
    }

    // pub fn check_alloc(&mut self, now: Timestamp, wnd: u64) -> Vec<DynamicPackage> {
    //     if self.flight() >= wnd {
    //         return Vec::new();
    //     }

    //     let mut packages = Vec::new();
    //     let mut flight = self.value_cache.flight;

    //     for block in &mut self.blocks {
    //         match &block.state {
    //             BlockState::Wait => {
    //                 block.state = BlockState::OnAir(now);
    //                 let post_flight = flight + block.len();
    //                 if post_flight > wnd {
    //                     break;
    //                 }
    //                 flight += block.len();
    //                 packages.push(DynamicPackage::from(block.to_session_data(now)));
    //             }, 
    //             _ => {

    //             }
    //         }
    //     }
    //     self.value_cache.flight = flight;
    //     packages
    // }

    pub fn check_wnd(
        &mut self, 
        stream: &PackageStream, 
        now: Timestamp, 
        timeout: Duration, 
        cwnd: u64, 
        packages: &mut Vec<DynamicPackage>, 
        logging: bool) {
        let _ = trace!("{} check wnd now:{} timeout:{}, wnd:{}", stream, now, timeout.as_micros(), cwnd);
        let mut flight = 0;
        let mut beyond_wnd = false; 
        for block in &mut self.blocks {
            if beyond_wnd {
                match &block.state {
                    BlockState::OnAir(_) => {
                        let _ = logging && {trace!("{} change block on air to wait for beyond wnd {}", stream, block); true};
                        block.state = BlockState::Wait;
                    }, 
                    BlockState::Wait => {
                        trace!("{} exits explore block for wait block beyond wnd {}", stream, block);
                        break;
                    }
                }
            } else {
                match &mut block.state {
                    BlockState::OnAir(send_time) => {
                        let post_flight = flight + block.data.len();
                        if post_flight as u64 > cwnd {
                            let _ = logging && {trace!("{} change block on air to wait for beyond wnd {}", stream, block); true};
                            beyond_wnd = true;
                            block.state = BlockState::Wait;
                        } else {
                            flight = post_flight;
                            if now > *send_time && Duration::from_micros(now - *send_time) > timeout {
                                *send_time = now;
                                packages.push(DynamicPackage::from(block.to_session_data(now)));
                                let _ = logging && {trace!("{} block resend for timeout {}", stream, block); true};
                            } else {
                                let _ = logging && {trace!("{} block wont resend for hasnt timeout {}", stream, block); true};
                            }
                        }
                    }, 
                    BlockState::Wait => {
                        let post_flight = flight + block.data.len();
                        if post_flight as u64 > cwnd {
                            trace!("{} exits explore block for wait block beyond wnd {}", stream, block);
                            break;
                        }
                        flight = post_flight;
                        packages.push(DynamicPackage::from(block.to_session_data(now)));
                        block.state = BlockState::OnAir(now);
                        let _ = logging && {trace!("{} will send {}", stream, block); true};
                    }
                }
            } 
        }

        self.value_cache.flight = flight;
    }

    pub fn check_timeout(&self, now: Timestamp, timeout: Duration) -> bool {
        now > self.value_cache.earliest_send_time 
            && Duration::from_micros(now - self.value_cache.earliest_send_time) > timeout
    }
}

