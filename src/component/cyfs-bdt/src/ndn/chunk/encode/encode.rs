use async_std::sync::Arc;
use cyfs_base::*;
use super::super::super::{
    channel::protocol::*, 
};

#[derive(Clone)]
pub enum ChunkEncoderState {
    Pending, 
    Ready, 
    Err(BuckyErrorCode)
}

#[async_trait::async_trait]
pub trait ChunkEncoder {
    fn chunk(&self) -> &ChunkId;
    fn state(&self) -> ChunkEncoderState;
    async fn wait_ready(&self) -> ChunkEncoderState;
    fn piece_of(&self, index: u32, buf: &mut [u8]) -> BuckyResult<usize>;
}

#[derive(Clone, Eq, PartialEq)]
pub enum ChunkDecoderState {
    Decoding(u32), 
    Ready, 
}

pub trait ChunkDecoder {
    fn chunk(&self) -> &ChunkId;
    fn state(&self) -> ChunkDecoderState;
    fn push_piece_data(&self, piece: &PieceData) -> (ChunkDecoderState, ChunkDecoderState);
    fn chunk_content(&self) -> Option<Arc<Vec<u8>>>;
}