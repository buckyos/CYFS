use log::*;
use std::{
    sync::{RwLock}, 
};
use async_std::{
    sync::Arc, 
    task, 
};
use cyfs_base::*;
use cyfs_raptorq::{
    RaptorEncoder as RawRaptorEncoder, 
    RaptorDecoder as RawRaptorDecoder, 
    DecodeStatus as RawRaptorStatus
};
use crate::{
    types::*, 
};
use super::super::super::{
    channel::protocol::*, 
};
use super::super::{
    storage::ChunkReader
};
use super::{
    encode::*
};


enum EncoderStateImpl {
    Pending(StateWaiter),
    Ready(RawRaptorEncoder), 
    Err(BuckyErrorCode)
}

struct RaptorEncoderImpl {
    chunk: ChunkId, 
    k: u16, 
    state: RwLock<EncoderStateImpl>
}

#[derive(Clone)]
pub struct RaptorEncoder(Arc<RaptorEncoderImpl>);

impl std::fmt::Display for RaptorEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RaptorEncoder{{chunk:{},k:{}}}", self.0.chunk, self.0.k)
    }
}

impl RaptorEncoder {
    pub fn from_reader(
        reader: Arc<Box<dyn ChunkReader>>, 
        chunk: &ChunkId) -> Self {
        
        let payload = PieceData::max_payload() - RawRaptorEncoder::extend_piece_size();
        let k = (chunk.len() + payload - 1) / payload;

        let arc_self = Self(Arc::new(RaptorEncoderImpl {
            chunk: chunk.clone(), 
            k: k as u16, 
            state: RwLock::new(EncoderStateImpl::Pending(StateWaiter::new())) 
        }));
        debug!("{} begin load from store： k {}, chunk_len {}, max_payload {}, extend_piece_size {}, payload {}", 
            arc_self, k, chunk.len(), PieceData::max_payload(), RawRaptorEncoder::extend_piece_size(), payload);
        {
            let arc_self = arc_self.clone();
            let chunk = chunk.clone();
            task::spawn(async move {
                let ret = reader.get(&chunk).await.map(|content| {
                    let padded_len = k * payload;
                    let mut buffer = vec![0u8; padded_len];
                    buffer[..chunk.len()].copy_from_slice(content.as_slice());
                    info!("{} chunk loaded from store, begin precode", arc_self);
                    let mut raw_encoder = RawRaptorEncoder::new(k as u32, payload as u16).unwrap();
                    let _ = raw_encoder.precode(buffer);
                    raw_encoder
                }); 
                let to_wake = {
                    let state =  &mut *arc_self.0.state.write().unwrap();
                    let to_wake = match state {
                        EncoderStateImpl::Pending(state_waiter) => {
                            info!("{} finish precode", arc_self);
                            let to_wake = state_waiter.transfer();
                            to_wake
                        }, 
                        _ => unreachable!()
                    };

                    match ret {
                        Ok(raw_encoder) => {
                            *state = EncoderStateImpl::Ready(raw_encoder);
                        }, 
                        Err(err) => {
                            error!("{} load chunk failed for {}", arc_self, err);
                            *state = EncoderStateImpl::Err(err.code());
                        }
                    }
                    to_wake
                };
                to_wake.wake();
            });
        }
        arc_self
    } 

    pub fn k(&self) -> u16 {
        self.0.k
    }
}

#[async_trait::async_trait]
impl ChunkEncoder for RaptorEncoder { 
    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn state(&self) -> ChunkEncoderState {
        match &*self.0.state.read().unwrap() {
            EncoderStateImpl::Ready(_) => ChunkEncoderState::Ready,
            EncoderStateImpl::Pending(_) => ChunkEncoderState::Pending, 
            EncoderStateImpl::Err(err) => ChunkEncoderState::Err(*err)
        }
    }

    async fn wait_ready(&self) -> ChunkEncoderState {
        let (state, waker) = match &mut *self.0.state.write().unwrap() {
            EncoderStateImpl::Ready(_) => (ChunkEncoderState::Ready, None),
            EncoderStateImpl::Pending(state_waiter) =>  (ChunkEncoderState::Pending, Some(state_waiter.new_waiter())), 
            EncoderStateImpl::Err(err) => (ChunkEncoderState::Err(*err), None),
        };
        if let Some(waker) = waker {
            StateWaiter::wait(waker, || self.state()).await
        } else {
            state
        }
    }

    fn piece_of(&self, index: u32, buf: &mut [u8]) -> BuckyResult<usize> {
        match &*self.0.state.read().unwrap() {
            EncoderStateImpl::Err(err) => Err(BuckyError::new(*err, "encoder in error")), 
            EncoderStateImpl::Pending(_) => Err(BuckyError::new(BuckyErrorCode::Pending, "raptor encoder pending on precode")), 
            EncoderStateImpl::Ready(raw_encoder) => {
                match raw_encoder.encode_raw(index, buf) {
                    Ok(len) => {
                        Ok(len)
                    }, 
                    Err(_) => {
                        let err = BuckyError::new(BuckyErrorCode::Failed, "raptor encode failed");
                        Err(err)
                    }
                }
            }
        }   
    }
}

struct DecodingState {
    pushed: u32, 
    raw_decoder: RawRaptorDecoder, 
}

enum DecoderStateImpl {
    Decoding(DecodingState), 
    Ready(Arc<Vec<u8>>)   
}


struct RaptorDecoderImpl {
    chunk: ChunkId, 
    k: usize, 
    state: RwLock<DecoderStateImpl>
}

#[derive(Clone)]
pub struct RaptorDecoder(Arc<RaptorDecoderImpl>);

impl std::fmt::Display for RaptorDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RaptorDecoder{{chunk:{},k:{}}}", self.0.chunk, self.0.k)
    }
}

impl RaptorDecoder {
    pub fn new(chunk: &ChunkId) -> Self {
        let payload = PieceData::max_payload() - RawRaptorEncoder::extend_piece_size();
        let k = (chunk.len() + payload - 1) / payload;

        Self(Arc::new(RaptorDecoderImpl {
            chunk: chunk.clone(), 
            k, 
            state: RwLock::new(DecoderStateImpl::Decoding(DecodingState {
                pushed: 0, 
                raw_decoder: RawRaptorDecoder::new(k as u32, payload as u16).unwrap()
            }))
        }))
    }

    pub fn k(&self) -> u16 {
        self.0.k as u16
    }
}


impl ChunkDecoder for RaptorDecoder {
    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn chunk_content(&self) -> Option<Arc<Vec<u8>>> {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(_) => None, 
            DecoderStateImpl::Ready(chunk) => Some(chunk.clone())
        }
    }

    fn state(&self) -> ChunkDecoderState {
        let state = &*self.0.state.read().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => ChunkDecoderState::Decoding(decoding.pushed), 
            DecoderStateImpl::Ready(_) => ChunkDecoderState::Ready
        } 
    }

    fn push_piece_data(&self, piece: &PieceData) -> (ChunkDecoderState, ChunkDecoderState) {
        let index = piece.desc.raptor_index(self.k()).unwrap();
        trace!("{} push piece seq {:?}", self, piece.desc);
        let state = &mut *self.0.state.write().unwrap();
        match state {
            DecoderStateImpl::Decoding(decoding) => {
                //FIXME: 这里应当不拷贝
                match decoding.raw_decoder.decode_raw(index, piece.data.clone()).unwrap() {
                    RawRaptorStatus::Keep => {
                        trace!("{} ingnore piece seq {} for duplicated piece", self, index);
                        (ChunkDecoderState::Decoding(decoding.pushed), ChunkDecoderState::Decoding(decoding.pushed))
                        
                    }, 
                    RawRaptorStatus::Step => {
                        let pushed = decoding.pushed;
                        decoding.pushed += 1;
                        trace!("{} added piece seq {}, total {}", self, index, decoding.pushed);
                        (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Decoding(decoding.pushed))
                    }, 
                    RawRaptorStatus::Done => {
                        let pushed = decoding.pushed;
                        trace!("{} added piece seq {}, total {}, got ready", self, index, decoding.pushed);
                        let piece_size = decoding.raw_decoder.piece_size();
                        let k = self.0.k;
                        let buf_len = k * piece_size;
                        let mut buffer = vec![0u8; buf_len];
                        for i in 0..k {
                            let _ = decoding.raw_decoder.retrieve_piece(i, &mut buffer[piece_size * i..piece_size * (i + 1)]).unwrap();
                        }
                        buffer.truncate(self.chunk().len());
                        
                        *state = DecoderStateImpl::Ready(Arc::new(buffer));
                        info!("{} got ready", self);
                        (ChunkDecoderState::Decoding(pushed), ChunkDecoderState::Ready)
                    }
                }
            }, 
            DecoderStateImpl::Ready(_) => {
                trace!("{} ingnore piece seq {} for decoder is ready", self, index);
                (ChunkDecoderState::Ready, ChunkDecoderState::Ready)
            }
        }
    }
}
