use std::{
    path::{PathBuf}, 
    sync::{Arc, RwLock},
    collections::LinkedList
};
use cyfs_base::*;
use crate::{
    ndn::{*, channel::{*, protocol::v0::*}}, 
    stack::{Stack}, 
};
use super::local_chunk_store::{LocalChunkWriter, LocalChunkListWriter};


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

    pub async fn id_streams(stack: &Stack, referer: String, remotes: Vec<DeviceId>) -> BuckyResult<Self> {
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


pub async fn local_chunk_writer(
    stack: &Stack, 
    chunk: &ChunkId,
    path: PathBuf 
) -> BuckyResult<LocalChunkWriter> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    Ok(LocalChunkWriter::new(
        path.to_owned(), None, chunk, 
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker()
    ))
}

pub async fn local_file_writer(
    stack: &Stack, 
    file: File, 
    path: PathBuf 
) -> BuckyResult<LocalChunkListWriter> {
    let _ = stack.ndn().chunk_manager().track_file(&file).await?;
    Ok(LocalChunkListWriter::new(
        path, 
        &ChunkListDesc::from_file(&file)?,  
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker()
    ))
}

pub fn get_download_task(
    stack: &Stack, 
    path: &str
) -> BuckyResult<Box<dyn DownloadTask>> {
    stack.ndn().root_task().download().sub_task(path)
        .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "no task in path"))
}

pub fn create_download_group(
    stack: &Stack, 
    path: String, 
) -> BuckyResult<Box<dyn DownloadTask>> {
    if let Some(group) = stack.ndn().root_task().download().sub_task(path.as_str()) {
        Ok(group)
    } else {
        let parts = path.split("::");
        let mut parent = stack.ndn().root_task().download().clone_as_task();
        
        for part in parts {
            if let Some(sub) = parent.sub_task(part) {
                parent = sub;
            } else {
                let sub = DownloadGroup::new(stack.config().ndn.channel.history_speed.clone(), None);
                parent.add_task(Some(part.to_owned()), sub.clone_as_task())?;
                parent = sub.clone_as_task();
            }
        }

        Ok(parent)
    }
}

fn create_download_task_owner(
    stack: &Stack, 
    group: Option<String>
) -> BuckyResult<(Box<dyn DownloadTask>, Option<String>)> {
    if let Some(group) = group {
        if group.len() == 0 {
            return Ok((stack.ndn().root_task().download().clone_as_task(), None));
        } 

        let mut parts: Vec<&str> = group.split("::").collect();
        if parts.len() == 0 {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid group path"))
        } 
        
        let last_part = if parts[parts.len() - 1].len() == 0 {
            None 
        } else {
            Some(parts[parts.len() - 1].to_owned())
        };

        parts.remove(parts.len() - 1);

        let group_path = parts.join("::"); 
        Ok((create_download_group(stack, group_path)?, last_part))
    } else {
        Ok((stack.ndn().root_task().download().clone_as_task(), None))
    }
}

pub async fn download_chunk(
    stack: &Stack, 
    chunk: ChunkId, 
    group: Option<String>, 
    context: impl DownloadContext
) -> BuckyResult<(Box<dyn DownloadTask>, ChunkTaskReader)> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    
    let (owner, path) = create_download_task_owner(stack, group)?;
    // 默认写到cache里面去
    let (task, reader) = ChunkTask::reader(
        stack.to_weak(), 
        chunk, 
        context.clone_as_context()
    );

    let _ = owner.add_task(path, task.clone_as_task())?;
    Ok((task.clone_as_task(), reader))
}

pub async fn download_chunk_list(
    stack: &Stack, 
    name: String, 
    chunks: &Vec<ChunkId>, 
    group: Option<String>, 
    context: impl DownloadContext, 
) -> BuckyResult<(Box<dyn DownloadTask>, ChunkListTaskReader)> {
    let chunk_list = ChunkListDesc::from_chunks(chunks);
    let _ = futures::future::try_join_all(chunks.iter().map(|chunk| stack.ndn().chunk_manager().track_chunk(chunk))).await?;

    let (owner, path) = create_download_task_owner(stack, group)?;
    let (task, reader) = ChunkListTask::reader(
        stack.to_weak(), 
        name, 
        chunk_list, 
        context.clone_as_context(), 
    );
    let _ = owner.add_task(path, task.clone_as_task())?;

    Ok((task.clone_as_task(), reader))
}


pub async fn track_file_in_path(
    stack: &Stack, 
    file: File, 
    path: PathBuf 
) -> BuckyResult<()> {
    let _ = stack.ndn().chunk_manager().track_file(&file).await?;
    LocalChunkListWriter::new(
        path, 
        &ChunkListDesc::from_file(&file)?,  
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker()
    ).track_path().await
}

pub async fn download_file(
    stack: &Stack, 
    file: File, 
    group: Option<String>, 
    context: impl DownloadContext
) -> BuckyResult<(Box<dyn DownloadTask>, ChunkListTaskReader)> {
    stack.ndn().chunk_manager().track_file(&file).await?;

    let (owner, path) = create_download_task_owner(stack, group)?;

    let chunk_list = ChunkListDesc::from_file(&file)?;
    let (task, reader) = ChunkListTask::reader(
        stack.to_weak(), 
        file.desc().file_id().to_string(), 
        chunk_list, 
        context.clone_as_context()
    );
    let _ = owner.add_task(path, task.clone_as_task())?;
    Ok((task.clone_as_task(), reader))
}


pub fn get_upload_task(
    stack: &Stack, 
    path: &str
) -> BuckyResult<Box<dyn UploadTask>> {
    stack.ndn().root_task().upload().sub_task(path)
        .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "no task in path"))
}

pub fn create_upload_group(
    stack: &Stack, 
    path: String
) -> BuckyResult<Box<dyn UploadTask>> {
    if let Some(group) = stack.ndn().root_task().upload().sub_task(path.as_str()) {
        Ok(group)
    } else {
        let parts = path.split("::");
        let mut parent = stack.ndn().root_task().upload().clone_as_task();
        
        for part in parts {
            if let Some(sub) = parent.sub_task(part) {
                parent = sub;
            } else {
                let sub = UploadGroup::new(stack.config().ndn.channel.history_speed.clone(), None);
                parent.add_task(Some(part.to_owned()), sub.clone_as_task())?;
                parent = sub.clone_as_task();
            }
        }

        Ok(parent)
    }
}

fn create_upload_task_owner(
    stack: &Stack, 
    group: Option<String>, 
) -> BuckyResult<(Box<dyn UploadTask>, Option<String>)> {
    if let Some(group) = group {
        if group.len() == 0 {
            return Ok((stack.ndn().root_task().upload().clone_as_task(), None));
        } 

        let mut parts: Vec<&str> = group.split("::").collect();
        if parts.len() == 0 {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid group path"))
        } 
        
        let last_part = if parts[parts.len() - 1].len() == 0 {
            None 
        } else {
            Some(parts[parts.len() - 1].to_owned())
        };

        parts.remove(parts.len() - 1);

        let group_path = parts.join("::"); 
        Ok((create_upload_group(stack, group_path)?, last_part))
    } else {
        Ok((stack.ndn().root_task().upload().clone_as_task(), None))
    }
}


pub async fn start_upload_task(
    stack: &Stack, 
    interest: &Interest, 
    to: &Channel, 
    owners: Vec<String>
) -> BuckyResult<Box<dyn UploadTask>> {
    let cache = stack.ndn().chunk_manager().create_cache(&interest.chunk);
    let desc = interest.prefer_type.fill_values(&interest.chunk);
    let encoder = cache.create_encoder(&desc);
    let session = to.upload(
        interest.chunk.clone(), 
        interest.session_id.clone(), 
        desc.clone(), 
        encoder)?;
   
    if owners.len() > 0 {
        for owner in owners {
            let (owner, path) = create_upload_task_owner(stack, Some(owner))?;
            let _ = owner.add_task(path, session.clone_as_task())?;
        }
    } else {
        stack.ndn().root_task().upload().add_task(None, session.clone_as_task())?;
    }
    // 加入到channel的 upload sessions中
   
    Ok(session.clone_as_task())
}
