use std::{
    sync::{atomic::{AtomicU16, Ordering::*}}, 
};
use async_std::{
    sync::Arc
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::super::super::{
    chunk::*, 
};
use super::super::{
    protocol::*
};
use super::provider::*; 



struct DownloadImpl {
    decoder: RaptorDecoder    
}

#[derive(Clone)]
pub struct RaptorDownload(Arc<DownloadImpl>);

impl RaptorDownload {
    pub fn new(decoder: RaptorDecoder) -> Self {
        Self(Arc::new(DownloadImpl {
            decoder
        }))
    }
}

impl DownloadSessionProvider for RaptorDownload {
    fn decoder(&self) -> &dyn ChunkDecoder {
        &self.0.decoder
    }
    
    fn clone_as_provider(&self) -> Box<dyn DownloadSessionProvider> {
        Box::new(self.clone())
    }

    fn on_time_escape(&self, _now: Timestamp) -> BuckyResult<()> {
        // do nothing
        Ok(())
    }

    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<bool/*finished*/> {
        let (_, next_state) = self.0.decoder.push_piece_data(piece);
        Ok(match next_state {
            ChunkDecoderState::Ready => true, 
            _ => false
        })
    }
}



struct UploadImpl {
    session_id: TempSeq, 
    encoder: RaptorEncoder, 
    next_index: AtomicU16, 
    sub: bool,
}

#[derive(Clone)]
pub struct RaptorUpload(Arc<UploadImpl>);

impl RaptorUpload {
    pub fn new(session_id: TempSeq, 
        encoder: RaptorEncoder, 
        start_index: u16,
        sub: bool) -> Self {
        Self(Arc::new(UploadImpl {
            session_id, 
            encoder, 
            next_index: AtomicU16::new(start_index),
            sub,
        }))
    }
}

impl UploadSessionProvider for RaptorUpload {
    fn state(&self) -> ChunkEncoderState {
        self.0.encoder.state()
    }

    fn clone_as_provider(&self) -> Box<dyn UploadSessionProvider> {
        Box::new(self.clone())
    }

    fn next_piece(
        &self, 
        buf: &mut [u8]
    ) -> BuckyResult<usize> {
        match self.0.encoder.state() {
            ChunkEncoderState::Err(err) => Err(BuckyError::new(err, "encoder failed")), 
            ChunkEncoderState::Pending => Ok(0), 
            ChunkEncoderState::Ready => {
                let index = if self.0.sub {
                    self.0.next_index.fetch_sub(1, SeqCst)
                } else {
                    self.0.next_index.fetch_add(1, SeqCst)
                };

                let buf_len = buf.len();
                let buf = PieceData::encode_header(
                    buf, 
                    &self.0.session_id,  
                    self.0.encoder.chunk(), 
                    &PieceDesc::Raptor(index as u32, self.0.encoder.k()))?;
                let header_len = buf_len - buf.len();
                let piece_len = self.0.encoder.piece_of(index as u32, buf).unwrap();
                trace!("will send data piece RaptorN{{chunk:{}, k:{}, index:{}, data:{}}}", self.0.encoder.chunk(), self.0.encoder.k(), index, piece_len);
                Ok(header_len + piece_len)
            }
        }
    }

    fn on_interest(&self, _interest: &Interest) -> BuckyResult<()> {
        Ok(())
    }

    fn on_piece_control(&self, _control: &PieceControl) -> BuckyResult<()> {
        Ok(())
    }
}