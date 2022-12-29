use std::{
    collections::{LinkedList}, 
    io::SeekFrom, 
    ops::Range
};
use async_std::{
    pin::Pin, 
    task::{Context, Poll},
    task
};

use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    types::*, 
    chunk::*,
    channel::{DownloadSession, protocol::v0::*}
};


#[derive(Clone, Debug)]
pub struct DownloadSourceFilter {
    pub exclude_target: Option<Vec<DeviceId>>, 
    pub include_target: Option<Vec<DeviceId>>, 
    pub include_codec: Option<Vec<ChunkCodecDesc>>, 
} 

impl Default for DownloadSourceFilter {
    fn default() -> Self {
        Self {
            exclude_target: None, 
            include_target: None, 
            include_codec: Some(vec![ChunkCodecDesc::Unknown])
        }
    } 
}

impl DownloadSourceFilter {
    pub fn fill_values(&mut self, chunk: &ChunkId) {
        self.include_codec = self.include_codec.as_ref().map(|include| include.iter().map(|codec| codec.fill_values(chunk)).collect());
    }

    pub fn check(&self, source: &DownloadSource<DeviceDesc>) -> bool {
        if let Some(exclude) = self.exclude_target.as_ref() {
            for target in exclude {
                if source.target.device_id().eq(target) {
                    return false;
                }
            }
        }

        if let Some(include_target) = self.include_target.as_ref() {
            let target_id = source.target.device_id();
            if include_target.iter().any(|include| target_id.eq(include)) {
                if let Some(include) = self.include_codec.as_ref() {
                    for codec in include {
                        if source.codec_desc.support_desc(codec) {
                            return true;
                        }
                    }
                } else {
                    return true;
                }
            }
            false
        } else if let Some(include) = self.include_codec.as_ref() {
            for codec in include {
                if source.codec_desc.support_desc(codec) {
                    return true;
                }
            }
            false
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
pub trait DownloadContext: Send + Sync {
    fn is_mergable(&self) -> bool {
        true
    }
    fn clone_as_context(&self) -> Box<dyn DownloadContext>;
    fn referer(&self) -> &str;
    // update time when context's sources changed
    async fn update_at(&self) -> Timestamp;
    async fn sources_of(
        &self, 
        filter: &DownloadSourceFilter, 
        limit: usize
    ) -> (
        LinkedList<DownloadSource<DeviceDesc>>, 
        /*context's current update_at*/
        Timestamp);
    fn on_new_session(
        &self, 
        _task: &dyn LeafDownloadTask, 
        _session: &DownloadSession, 
        /*session created based on context's update_at*/
        _update_at: Timestamp
    ) {}
    // called when tried all source in context but task didn't finish  
    fn on_drain(
        &self, 
        _task: &dyn LeafDownloadTask, 
        /*event trigered based on context's update_at*/
        _update_at: Timestamp) {}
}

#[derive(Clone, Debug)]
pub struct DownloadSource<T: std::fmt::Debug + Clone + Send + Sync> {
    pub target: T, 
    pub codec_desc: ChunkCodecDesc, 
}

impl Into<DownloadSource<DeviceId>> for DownloadSource<DeviceDesc> {
    fn into(self) -> DownloadSource<DeviceId> {
        DownloadSource {
            target: self.target.device_id(), 
            codec_desc: self.codec_desc, 
        }
    }
}


#[derive(Clone, Copy)]
pub enum DownloadTaskPriority {
    Backgroud, 
    Normal, 
    Realtime(u32/*min speed*/),
}

impl Default for DownloadTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}





#[async_trait::async_trait]
pub trait DownloadTask: NdnTask {
    fn clone_as_download_task(&self) -> Box<dyn DownloadTask>;

    async fn wait_user_canceled(&self) -> BuckyError;

    fn add_task(&self, _path: Option<String>, _sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "no implement"))
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask>> {
        None
    }
    fn on_post_add_to_root(&self, _abs_path: String) {

    }

    fn calc_speed(&self, when: Timestamp) -> u32;
   
    fn downloaded(&self) -> u64 {
        0
    }
}


#[async_trait::async_trait]
pub trait LeafDownloadTask: DownloadTask {
    fn priority(&self) -> DownloadTaskPriority {
        DownloadTaskPriority::default()
    }
    fn clone_as_leaf_task(&self) -> Box<dyn LeafDownloadTask>;
    fn abs_group_path(&self) -> Option<String>;
    fn context(&self) -> &dyn DownloadContext;
    fn finish(&self);
}


pub struct DownloadTaskReader {
    cache: ChunkCache, 
    offset: usize,
    task: Box<dyn LeafDownloadTask>
}


pub trait DownloadTaskSplitRead: std::io::Seek {
    fn poll_split_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<Option<(ChunkCache, Range<usize>)>>>;
}

impl std::fmt::Display for DownloadTaskReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DownloadTaskReader{{chunk:{}}}", self.cache.chunk())
    }
}

impl DownloadTaskReader {
    pub fn new(cache: ChunkCache, task: Box<dyn LeafDownloadTask>) -> Self {
        Self {
            cache, 
            offset: 0, 
            task
        }
    }

    pub fn task(&self) -> &dyn LeafDownloadTask {
        self.task.as_ref()
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn cache(&self) -> &ChunkCache {
        &self.cache
    }
}

impl DownloadTaskSplitRead for DownloadTaskReader {
    fn poll_split_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<Option<(ChunkCache, Range<usize>)>>> {
        let pined = self.get_mut();
        trace!("{} split_read: {} offset: {}", pined, buffer.len(), pined.offset);
        if let NdnTaskState::Error(err) = pined.task.state() {
            trace!("{} split_read: {} offset: {} error: {}", pined, buffer.len(), pined.offset, err);
            return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::Other, BuckyError::new(err, ""))));
        } 
        if let Some(range) = pined.cache.exists(pined.offset..pined.offset + buffer.len()) {
            trace!("{} split_read: {} offset: {} exists {:?}", pined, buffer.len(), pined.offset, range);
            let (desc, mut offset) = PieceDesc::from_stream_offset(PieceData::max_payload(), range.start as u32);
            let (mut index, len) = desc.unwrap_as_stream();
            let mut read = 0;
            let result = loop {
                match pined.cache.stream().sync_try_read(
                    &PieceDesc::Range(index, len), 
                    offset as usize, 
                    &mut buffer[read..]) {
                    Ok(this_read) => {
                        read += this_read;
                        if this_read == 0 
                            || read >= buffer.len() {
                            pined.offset += read;
                            break Ok(read);
                        }
                        index += 1;
                        offset = 0;
                    },
                    Err(err) => {
                        break Err(std::io::Error::new(std::io::ErrorKind::Other, err))
                    }
                }
            };
            Poll::Ready(result.map(|read| Some((pined.cache.clone(), range.start..range.start + read))))
        } else {
            let waker = cx.waker().clone();
            let cache = pined.cache.clone();
            let task = pined.task.clone_as_download_task();
            let range = pined.offset..pined.offset + buffer.len();
            task::spawn(async move {
                let _ = cache.wait_exists(range, || task.wait_user_canceled()).await;
                waker.wake();
            });
            Poll::Pending
        }
    }
}

impl std::io::Seek for DownloadTaskReader {
    fn seek(
        self: &mut Self,
        pos: SeekFrom,
    ) -> std::io::Result<u64> {
        let len = self.cache.chunk().len();
        let new_offset = match pos {
            SeekFrom::Start(offset) => len.min(offset as usize), 
            SeekFrom::Current(offset) => {
                let offset = (self.offset as i64) + offset;
                let offset = offset.max(0);
                len.min(offset as usize)
            },
            SeekFrom::End(offset) => {
                let offset = (len as i64) + offset;
                let offset = offset.max(0);
                len.min(offset as usize)
            }
        };
        self.offset = new_offset;

        Ok(new_offset as u64)   
    }
}

impl async_std::io::Read for DownloadTaskReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        self.poll_split_read(cx, buffer).map(|result| result.map(|r| if let Some((_, r)) = r {
            r.end - r.start
        } else {
            0
        }))
    }
}
