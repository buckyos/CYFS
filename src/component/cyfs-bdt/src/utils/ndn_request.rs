use std::{
    path::{PathBuf}, 
};
use cyfs_base::*;
use crate::{
    ndn::{*, channel::{*, protocol::v0::*}}, 
    stack::{Stack}, 
};
use super::local_chunk_store::{LocalChunkWriter, LocalChunkListWriter};

pub struct NdnRequester {
    stack: Stack, 
    task: Box<dyn DownloadTask>
}

impl NdnRequester {
    pub fn new(
        stack: Stack, 
        root: ObjectId, 
        context: SingleDownloadContext
    ) -> BuckyResult<Self> {
        Self {
            stack
        }
    }

    pub fn close(&self) {

    }

    pub fn cancel(&self) {

    }

    pub async fn get(
        &self, 
        object: ObjectId, 
        inner_path: String
    ) -> BuckyResult<(
        Box<dyn std::io::Seek + async_std::io::Read>, 
        Option<Box<dyn DownloadTask>>)> {
        unimplemented!()
    } 
}