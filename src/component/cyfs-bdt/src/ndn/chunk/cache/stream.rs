use std::{
    sync::{Arc, RwLock}, 
    ops::Range, 
    io::SeekFrom, 
    collections::BTreeMap
};
use async_std::{
    task
};
use once_cell::sync::OnceCell;
use cyfs_base::*;
use crate::{
    interface::udp::MTU, 
    types::*
};
use super::super::super::{
    types::*, 
    channel::{protocol::v0::*}
};
use super::{
    encode::*, 
    raw_cache::*
};


struct StateImpl {
    raw_cache: OnceCell<Box<dyn RawCache>>, 
    indices: IncomeIndexQueue, 
    waiters: BTreeMap::<u32, StateWaiter>
}

struct CacheImpl {
    chunk: ChunkId, 
    state: RwLock<StateImpl>
} 

#[derive(Clone)]
pub struct ChunkStreamCache(Arc<CacheImpl>);


impl ChunkStreamCache {
    pub fn new(chunk: &ChunkId) -> Self {
        Self(Arc::new((CacheImpl {
            chunk: chunk.clone(),
            state: RwLock::new(StateImpl {
                raw_cache: OnceCell::new(), 
                indices: IncomeIndexQueue::new(chunk.len() as u32), 
                waiters: BTreeMap::new()
            })
        })))
    }

    pub fn load(
        &self, 
        finished: bool, 
        raw_cache: Box<dyn RawCache>, 
    ) -> BuckyResult<()> {

        let waiters = {
            let mut state = self.0.state.write().unwrap();
            match state.raw_cache.set(raw_cache) {
                Ok(_) => {
                    if finished {
                        state.indices.push(0..self.chunk().len() as u32);
                        let mut waiters = Default::default();
                        std::mem::swap(&mut waiters, &mut state.waiters);
                        Ok(waiters.into_values().collect())
                    } else {
                        Ok(vec![])
                    }
                },
                Err(_) => Err(BuckyError::new(BuckyErrorCode::ErrorState, "loaded"))
            }
        }?;
        
        for waiter in waiters {
            waiter.wake();
        }

        Ok(())
    }

    pub fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn require_index(&self, desc: &ChunkEncodeDesc) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)> {
        let (start, end, step) = desc.unwrap_as_stream();
        self.0.state.read().unwrap().indices.require(start, end, step)
    }

    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<PushIndexResult> {
        let (index, range) = piece.desc.stream_piece_range(self.chunk());
        let index_result = self.0.state.read().unwrap().indices.try_push(index..index + 1);
        if !index_result.pushed() {
            return Ok(index_result);
        }

        let mut writer = {
            let state = self.0.state.read().unwrap();
            state.raw_cache.get().unwrap().sync_writer()
        }?;

        if range.start == writer.seek(SeekFrom::Start(range.start))? {
            let len = (range.end - range.start) as usize;
            if len == writer.write(&piece.data[..len])? {
                let (result, waiter) = {
                    let mut state = self.0.state.write().unwrap();
                    let result = state.indices.push(index..index + 1);
                    (result, state.waiters.remove(&index))
                };
                if let Some(waiter) = waiter {
                    waiter.wake();
                }
                Ok(result)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
        }
    }

    fn exists(&self, index: u32) -> BuckyResult<bool> {
        self.0.state.read().unwrap().indices.exists(index)
    }

    pub async fn wait_exists<T: futures::Future<Output=BuckyError>>(&self, index: u32, abort: T) -> BuckyResult<()> {
        let waiter = {
            let mut state = self.0.state.write().unwrap();
            match state.indices.exists(index) {
                Ok(exists) => {
                    if exists {
                        return Ok(());
                    }
                }, 
                Err(err) => {
                    return Err(err); 
                }
            }

            if let Some(waiters) = state.waiters.get_mut(&index) {
                waiters.new_waiter()
            } else {
                let mut waiters = StateWaiter::new();
                let waiter = waiters.new_waiter();
                state.waiters.insert(index, waiters);
                waiter
            }
        };
        StateWaiter::abort_wait(abort, waiter, || ()).await
    }

    pub async fn async_read<T: futures::Future<Output=BuckyError>>(
        &self, 
        piece_desc: &PieceDesc, 
        offset_in_piece: usize,  
        buffer: &mut [u8], 
        abort: T
    ) -> BuckyResult<usize> {
        let (index, range) = piece_desc.stream_piece_range(self.chunk());
        if self.wait_exists(index, abort).await.is_err() {
            return Ok(0);
        }
        let raw_cache = self.0.state.read().unwrap().raw_cache.get().unwrap().clone_as_raw_cache();
        let mut reader = raw_cache.async_reader().await?;
        use async_std::io::prelude::*;
        let start = range.start + offset_in_piece as u64;
        if start == reader.seek(SeekFrom::Start(start)).await? {
            let len = (range.end - start) as usize;
            if len == reader.read(&mut buffer[..len]).await? {
                Ok(len)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
        }
    }


    fn sync_try_read(
        &self, 
        piece_desc: &PieceDesc, 
        offset_in_piece: usize,  
        buffer: &mut [u8]
    ) -> BuckyResult<usize> {
        let (index, range) = piece_desc.stream_piece_range(self.chunk());
        match self.exists(index) {
            Ok(exists) => {
                if !exists {
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, "not exists"));
                }
            }, 
            Err(_) => {
                return Ok(0);
            }
        }
        let raw_cache = self.0.state.read().unwrap().raw_cache.get().unwrap().clone_as_raw_cache();
        let mut reader = raw_cache.sync_reader()?;
        use std::io::{Read, Seek};
        let start = range.start + offset_in_piece as u64;
        if start == reader.seek(SeekFrom::Start(start))? {
            let len = (range.end - start) as usize;
            if len == reader.read(&mut buffer[..len])? {
                Ok(len)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
        }
    }

    async fn async_try_read(
        &self, 
        piece_desc: &PieceDesc, 
        offset_in_piece: usize,  
        buffer: &mut [u8]
    ) -> BuckyResult<usize> {
        let (index, range) = piece_desc.stream_piece_range(self.chunk());
        match self.exists(index) {
            Ok(exists) => {
                if !exists {
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, "not exists"));
                }
            }, 
            Err(_) => {
                return Ok(0);
            }
        }
        let raw_cache = self.0.state.read().unwrap().raw_cache.get().unwrap().clone_as_raw_cache();
        let mut reader = raw_cache.async_reader().await?;
        use async_std::io::prelude::*;
        let start = range.start + offset_in_piece as u64;
        if start == reader.seek(SeekFrom::Start(start)).await? {
            let len = (range.end - start) as usize;
            if len == reader.read(&mut buffer[..len]).await? {
                Ok(len)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidInput, "len mismatch"))
        }
    }
}



struct DecoderImpl {
    chunk: ChunkId, 
    desc: ChunkEncodeDesc,  
    cache: ChunkStreamCache, 
}

#[derive(Clone)]
pub struct StreamDecoder(Arc<DecoderImpl>);


impl std::fmt::Display for StreamDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamDecoder{{chunk:{}}}", self.chunk())
    }
}

impl StreamDecoder {
    pub fn new(
        chunk: &ChunkId, 
        desc: &ChunkEncodeDesc, 
        cache: ChunkStreamCache
    ) -> Self {
        Self(Arc::new(DecoderImpl {
            chunk: chunk.clone(), 
            desc: desc.clone(), 
            cache, 
        }))
    }
}

impl ChunkDecoder for StreamDecoder {
    fn clone_as_decoder(&self) -> Box<dyn ChunkDecoder> {
        Box::new(self.clone())
    }

    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }

    fn desc(&self) -> &ChunkEncodeDesc {
        &self.0.desc
    }

    fn require_index(&self) -> Option<(Option<u32>, Option<Vec<Range<u32>>>)> {
        self.0.cache.require_index(self.desc())
    }

    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<PushIndexResult> {
        trace!("{} push piece desc {:?}", self, piece.desc);
        let (start, end, step) = self.desc().unwrap_as_stream();
        let (index, range) = piece.desc.unwrap_as_stream();
        if index < start || index >= end {
            return Ok(PushIndexResult {
                valid: false, 
                exists: false, 
                finished: false
            });
        }

        let result = self.0.cache.push_piece_data(piece)?;
        if result.pushed() {
            if self.0.cache.require_index(self.desc()).is_none() {
                Ok(PushIndexResult { 
                    valid: true, 
                    exists: false,
                    finished: true })
            } else {
                Ok(result)
            }
        } else {
            Ok(result)
        }
    }

}


enum EncoderPendingState {
    None, 
    Pending(PieceDesc), 
    // FIXME: may not allocated every time
    Waiting(PieceDesc, BuckyResult<Vec<u8>>)
}

struct EncoderStateImpl {
    pending: EncoderPendingState, 
    indices: OutcomeIndexQueue, 
}

struct EncoderImpl {
    desc: ChunkEncodeDesc, 
    cache: ChunkStreamCache,  
    state: RwLock<EncoderStateImpl>
}

#[derive(Clone)]
pub struct StreamEncoder(Arc<EncoderImpl>);

impl std::fmt::Display for StreamEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamEncoder{{chunk:{},desc:{:?}}}", self.chunk(), self.desc())
    }
}


impl StreamEncoder {
    pub fn new(
        cache: ChunkStreamCache, 
        desc: &ChunkEncodeDesc
    ) -> Self {
        let (start, end, step) = desc.unwrap_as_stream();
        Self(Arc::new(EncoderImpl {
            desc: desc.clone(), 
            cache, 
            state: RwLock::new(EncoderStateImpl {
                pending: EncoderPendingState::None, 
                indices: OutcomeIndexQueue::new(start, end, step)
            })
        }))
    }

    fn cache(&self) -> &ChunkStreamCache {
        &self.0.cache
    }

    async fn async_next_piece(&self, piece_desc: PieceDesc) {
        let mut buffer = vec![0u8; MTU];
        let result = self.cache().async_try_read(&piece_desc, 0, &mut buffer[..]).await;
        let mut state = self.0.state.write().unwrap();
        if let EncoderPendingState::Pending(pending_desc) = &state.pending {
            if pending_desc.eq(&piece_desc) {
                state.pending = EncoderPendingState::Waiting(piece_desc, result.map(|len| {
                    buffer.truncate(len);
                    buffer
                }));
            }
        }
    }
}

impl ChunkEncoder for StreamEncoder {
    fn clone_as_encoder(&self) -> Box<dyn ChunkEncoder> {
        Box::new(self.clone())
    }

    fn chunk(&self) -> &ChunkId {
        self.cache().chunk()
    }

    fn desc(&self) -> &ChunkEncodeDesc {
        &self.0.desc
    }

    fn next_piece(&self, session_id: &TempSeq, buf: &mut [u8]) -> BuckyResult<usize> {
        let mut state = self.0.state.write().unwrap();
        match &mut state.pending {
            EncoderPendingState::Pending(_) => Ok(0), 
            EncoderPendingState::Waiting(piece_desc, _result) => {
                let mut result = Err(BuckyError::new(BuckyErrorCode::Ok, ""));
                std::mem::swap(&mut result, _result);
                let piece_desc = piece_desc.clone(); 
                state.pending = EncoderPendingState::None;
                match result {
                    Ok(buffer) => {
                        let (index, _) = piece_desc.unwrap_as_stream();
                        if state.indices.next() == Some(index) {
                            let _ = state.indices.pop_next();
                            let buf_len = buf.len();
                            let buf = PieceData::encode_header(
                                buf, 
                                session_id,
                                self.chunk(), 
                                &piece_desc)?;
                            let header_len = buf_len - buf.len();
                            buf[..buffer.len()].copy_from_slice(&buffer[..]);
                            let piece_len = header_len + buffer.len();
                            Ok(piece_len)
                        } else {
                            Ok(0)
                        }
                    }, 
                    Err(err) => {
                        Err(err)
                    }
                }
            }, 
            EncoderPendingState::None => {
                if let Some(index) = state.indices.next() {
                    if self.cache().exists(index).unwrap() {
                        let (_, _, step) = self.desc().unwrap_as_stream();
                        let piece_desc = PieceDesc::Range(index, step.abs() as u16);
                        let buf_len = buf.len();
                        let buf = PieceData::encode_header(
                            buf, 
                            session_id,
                            self.chunk(), 
                            &piece_desc)?;
                        let header_len = buf_len - buf.len();
                        match self.cache().sync_try_read(&piece_desc, 0, buf) {
                            Ok(len) => {
                                let _ = state.indices.pop_next();
                                Ok(header_len + len)
                            }, 
                            Err(err) => {
                                if BuckyErrorCode::UnSupport == err.code() {
                                    state.pending = EncoderPendingState::Pending(piece_desc.clone());
                                    let encoder = self.clone();
                                    task::spawn(async move {
                                        encoder.async_next_piece(piece_desc).await;
                                    });
                                    Ok(0)
                                } else {
                                    Err(err)
                                }
                            }
                        }
                    } else {
                        Ok(0)
                    }
                } else {
                    Ok(0)
                }
            }
        }
    }

    fn reset(&self) {
        let mut state = self.0.state.write().unwrap();
        state.indices.reset();

        match &state.pending {
            EncoderPendingState::Pending(next_desc) => {
                let (index, _) = next_desc.unwrap_as_stream();
                if state.indices.next() != Some(index) {
                    state.pending = EncoderPendingState::None;
                }
            },
            EncoderPendingState::Waiting(next_desc, _) => {
                let (index, _) = next_desc.unwrap_as_stream();
                if state.indices.next() != Some(index) {
                    state.pending = EncoderPendingState::None;
                }
            },
            _ => {}
        }
    }

    fn merge(&self, max_index: u32, lost_index: Vec<Range<u32>>) {
        let mut state = self.0.state.write().unwrap();
        state.indices.reset();

        match &state.pending {
            EncoderPendingState::Pending(next_desc) => {
                let (index, _) = next_desc.unwrap_as_stream();
                if state.indices.next() != Some(index) {
                    state.pending = EncoderPendingState::None;
                }
            },
            EncoderPendingState::Waiting(next_desc, _) => {
                let (index, _) = next_desc.unwrap_as_stream();
                if state.indices.next() != Some(index) {
                    state.pending = EncoderPendingState::None;
                }
            },
            _ => {}
        }
    }
}