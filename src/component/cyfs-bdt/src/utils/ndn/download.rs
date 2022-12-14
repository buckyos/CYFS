use std::{
    sync::{Arc, RwLock},
    collections::LinkedList
};
use cyfs_base::*;
use crate::{
    ndn::{*}, 
    stack::{Stack}, 
};


struct SingleContextImpl {
    referer: String, 
    sources: RwLock<LinkedList<DownloadSource>>, 
}

#[derive(Clone)]
pub struct SingleDownloadContext(Arc<SingleContextImpl>);

impl Default for SingleDownloadContext {
    fn default() -> Self {
        Self(Arc::new(SingleContextImpl {
            referer: "".to_owned(), 
            sources: RwLock::new(Default::default()), 
        }))
    }
}

impl SingleDownloadContext {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn new(referer: String) -> Self {
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(Default::default())
        }))
    }

    pub fn desc_streams(referer: String, remotes: Vec<DeviceDesc>) -> Self {
        let mut sources = LinkedList::new();
        for remote in remotes {
            sources.push_back(DownloadSource {
                target: remote, 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
            });
        } 
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        }))
    }

    pub async fn id_streams(stack: &Stack, referer: String, remotes: &[DeviceId]) -> BuckyResult<Self> {
        let mut sources = LinkedList::new();
        for remote in remotes {
            let device = stack.device_cache().get(&remote).await
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "device desc not found"))?;
            sources.push_back(DownloadSource {
                target: device.desc().clone(), 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
            });
        } 
        Ok(Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        })))
    }

    pub fn add_source(&self, source: DownloadSource) {
        self.0.sources.write().unwrap().push_back(source);
    }
}


impl DownloadContext for SingleDownloadContext {
    fn clone_as_context(&self) -> Box<dyn DownloadContext> {
        Box::new(self.clone())
    }

    fn referer(&self) -> &str {
        self.0.referer.as_str()
    }

    fn source_exists(&self, target: &DeviceId, encode_desc: &ChunkEncodeDesc) -> bool {
        let sources = self.0.sources.read().unwrap();
        sources.iter().find(|s| s.target.device_id().eq(target) && s.encode_desc.support_desc(encode_desc)).is_some()
    }

    fn sources_of(&self, filter: Box<dyn Fn(&DownloadSource) -> bool>, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut count = 0;
        let sources = self.0.sources.read().unwrap();
        for source in sources.iter() {
            if (*filter)(source) {
                result.push_back(DownloadSource {
                    target: source.target.clone(), 
                    encode_desc: source.encode_desc.clone(), 
                });
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }
        return result;
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

