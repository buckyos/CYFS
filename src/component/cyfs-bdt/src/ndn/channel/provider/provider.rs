use cyfs_base::*;
use crate::{
    types::*
};
use super::super::super::{
    chunk::*
};
use super::super::{
    protocol::*
};
pub trait DownloadSessionProvider: Send + Sync {
    fn decoder(&self) -> &dyn ChunkDecoder;
    fn clone_as_provider(&self) -> Box<dyn DownloadSessionProvider>;
    fn on_time_escape(&self, now: Timestamp) -> BuckyResult<()>;
    fn push_piece_data(&self, piece: &PieceData) -> BuckyResult<bool>;
}


pub trait UploadSessionProvider: Send + Sync {
    fn state(&self) -> ChunkEncoderState;
    fn clone_as_provider(&self) -> Box<dyn UploadSessionProvider>;
    fn next_piece(&self, buf: &mut [u8]) -> BuckyResult<usize>;
    fn on_interest(&self, interest: &Interest) -> BuckyResult<()>;
    fn on_piece_control(&self, control: &PieceControl) -> BuckyResult<()>;
}
