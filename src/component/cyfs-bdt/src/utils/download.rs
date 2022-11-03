use std::{
    path::{PathBuf}, 
};
use cyfs_base::*;
use crate::{
    ndn::{*, channel::{*, protocol::v0::*}}, 
    stack::{Stack}, 
};
use super::local_chunk_store::{LocalChunkWriter, LocalChunkListWriter};

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
    context: Option<SingleDownloadContext>
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
                let sub = DownloadGroup::new(stack.config().ndn.channel.history_speed.clone(), None, context.clone().unwrap_or(parent.context().clone()));
                parent.add_task(Some(part.to_owned()), sub.clone_as_task())?;
                parent = sub.clone_as_task();
            }
        }

        Ok(parent)
    }
}

fn create_download_task_owner(
    stack: &Stack, 
    group: Option<String>, 
    context: Option<SingleDownloadContext>, 
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
        Ok((create_download_group(stack, group_path, context.clone())?, last_part))
    } else {
        Ok((stack.ndn().root_task().download().clone_as_task(), None))
    }
}

pub async fn download_chunk(
    stack: &Stack, 
    chunk: ChunkId, 
    group: Option<String>, 
    context: Option<SingleDownloadContext>
) -> BuckyResult<ChunkTask> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    
    let (owner, path) = create_download_task_owner(stack, group, context.clone())?;
    // 默认写到cache里面去
    let task = ChunkTask::new(
        stack.to_weak(), 
        chunk, 
        context.unwrap_or(owner.context().clone()), 
    );

    let _ = owner.add_task(path, task.clone_as_task())?;
    Ok(task)
}

pub async fn download_chunk_list(
    stack: &Stack, 
    name: String, 
    chunks: &Vec<ChunkId>, 
    group: Option<String>, 
    context: Option<SingleDownloadContext>, 
) -> BuckyResult<ChunkListTask> {
    let chunk_list = ChunkListDesc::from_chunks(chunks);
    let _ = futures::future::try_join_all(chunks.iter().map(|chunk| stack.ndn().chunk_manager().track_chunk(chunk))).await?;

    let (owner, path) = create_download_task_owner(stack, group, context.clone())?;
    let task = ChunkListTask::new(
        stack.to_weak(), 
        name, 
        chunk_list, 
        context.unwrap_or(owner.context().clone()), 
    );
    let _ = owner.add_task(path, task.clone_as_task())?;

    Ok(task)
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
    context: Option<SingleDownloadContext>
) -> BuckyResult<FileTask> {
    stack.ndn().chunk_manager().track_file(&file).await?;

    let (owner, path) = create_download_task_owner(stack, group, context.clone())?;

    let chunk_list = ChunkListDesc::from_file(&file)?;
    let task = FileTask::new(
        stack.to_weak(), 
        file, 
        Some(chunk_list), 
        context.unwrap_or(owner.context().clone()), 
    );
    let _ = owner.add_task(path, task.clone_as_task())?;
    Ok(task)
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
