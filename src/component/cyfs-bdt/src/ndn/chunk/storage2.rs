use std::{
    ops::Range, 
    collections::LinkedList
};
use async_trait::async_trait;
use async_std::{ 
    sync::Arc, 
    io::{prelude::*, Cursor}, 
    pin::Pin, 
    task::{Context, Poll}
};
use cyfs_base::*;
use cyfs_util::AsyncReadWithSeek;

#[async_trait]
pub trait ChunkWriter2: 'static + std::fmt::Display + Send + Sync {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter2>;
    async fn write(&self, chunk: &ChunkId, offset: u64, buffer: &[u8]) -> BuckyResult<usize>;
    async fn finish(&self) -> BuckyResult<()>;
    async fn err(&self, e: BuckyErrorCode) -> BuckyResult<()>;
}


