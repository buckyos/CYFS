use std::{
    sync::{Arc, RwLock},
    collections::LinkedList
};
use cyfs_base::*;
use crate::{
    types::*, 
    ndn::{*}, 
    stack::{Stack}, 
};

struct SampleContextSources {
    update_at: Timestamp, 
    sources: LinkedList<DownloadSource<DeviceDesc>>, 
}

struct SampleContextImpl {
    referer: String, 
    sources: RwLock<SampleContextSources>, 
}

#[derive(Clone)]
pub struct SampleDownloadContext(Arc<SampleContextImpl>);

impl Default for SampleDownloadContext {
    fn default() -> Self {
        Self::new("".to_owned())
    }
}

impl SampleDownloadContext {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn new(referer: String) -> Self {
        Self(Arc::new(SampleContextImpl {
            referer, 
            sources: RwLock::new(SampleContextSources {
                update_at: bucky_time_now(), 
                sources: Default::default()
            })
        }))
    }

    pub fn desc_streams(referer: String, remotes: Vec<DeviceDesc>) -> Self {
        let mut sources = LinkedList::new();
        for remote in remotes {
            sources.push_back(DownloadSource {
                target: remote, 
                codec_desc: ChunkCodecDesc::Stream(None, None, None), 
            });
        } 
        Self(Arc::new(SampleContextImpl {
            referer, 
            sources: RwLock::new(SampleContextSources { update_at: bucky_time_now(),  sources})
        }))
    }

    pub async fn id_streams(stack: &Stack, referer: String, remotes: &[DeviceId]) -> BuckyResult<Self> {
        let mut sources = LinkedList::new();
        for remote in remotes {
            let device = stack.device_cache().get(&remote).await
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "device desc not found"))?;
            sources.push_back(DownloadSource {
                target: device.desc().clone(), 
                codec_desc: ChunkCodecDesc::Stream(None, None, None), 
            });
        } 
        Ok(Self(Arc::new(SampleContextImpl {
            referer, 
            sources: RwLock::new(SampleContextSources{ update_at: bucky_time_now(), sources })
        })))
    }

    pub fn add_source(&self, source: DownloadSource<DeviceDesc>) {
        let mut sources = self.0.sources.write().unwrap();
        sources.update_at = bucky_time_now();
        sources.sources.push_back(source);
    }
}

#[async_trait::async_trait]
impl DownloadContext for SampleDownloadContext {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn referer(&self) -> &str {
        self.0.referer.as_str()
    }

    fn update_at(&self) -> Timestamp {
        self.0.sources.read().unwrap().update_at
    }

    async fn sources_of(&self, filter: &DownloadSourceFilter, limit: usize) -> (LinkedList<DownloadSource<DeviceDesc>>, Timestamp) {
        let mut result = LinkedList::new();
        let mut count = 0;
        let sources = self.0.sources.read().unwrap();
        for source in sources.sources.iter() {
            if filter.check(source) {
                result.push_back(DownloadSource {
                    target: source.target.clone(), 
                    codec_desc: source.codec_desc.clone(), 
                });
                count += 1;
                if count >= limit {
                    return (result, sources.update_at);
                }
            }
        }
        return (result, sources.update_at);
    }
}



pub async fn download_chunk(
    stack: &Stack, 
    chunk: ChunkId, 
    group: Option<String>, 
    context: impl DownloadContext
) -> BuckyResult<(String, ChunkTaskReader)> {
    let (task, reader) = ChunkTask::reader(
        stack.to_weak(), 
        chunk, 
        context.clone_as_context()
    );
    let path = stack.ndn().root_task().download().add_task(group.unwrap_or_default(), &task)?;
    Ok((path, reader))
}

pub async fn download_chunk_list(
    stack: &Stack, 
    name: String, 
    chunks: &Vec<ChunkId>, 
    group: Option<String>, 
    context: impl DownloadContext, 
) -> BuckyResult<(String, ChunkListTaskReader)> {
    let chunk_list = ChunkListDesc::from_chunks(chunks);
   
    let (task, reader) = ChunkListTask::reader(
        stack.to_weak(), 
        name, 
        chunk_list, 
        context.clone_as_context(), 
    );
    let path = stack.ndn().root_task().download().add_task(group.unwrap_or_default(), &task)?;
    
    Ok((path, reader))
}


pub async fn download_file(
    stack: &Stack, 
    file: File, 
    group: Option<String>, 
    context: impl DownloadContext
) -> BuckyResult<(String, ChunkListTaskReader)> {
    let chunk_list = ChunkListDesc::from_file(&file)?;
    let (task, reader) = ChunkListTask::reader(
        stack.to_weak(), 
        file.desc().file_id().to_string(), 
        chunk_list, 
        context.clone_as_context()
    );
    let path = stack.ndn().root_task().download().add_task(group.unwrap_or_default(), &task)?;
    Ok((path, reader))
}

