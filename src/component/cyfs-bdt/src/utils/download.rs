use std::{
    path::{Path, PathBuf}, 
    ops::Range
};
use async_std::{
    sync::Arc, 
    io::{prelude::*}, 
    fs::OpenOptions,
};
use cyfs_base::*;
use crate::{
    ndn::*, 
    stack::{Stack, WeakStack}, 
};
use super::local_chunk_store::{LocalChunkWriter, LocalChunkListWriter};

pub async fn track_chunk_in_path(
    stack: &Stack, 
    chunk: &ChunkId,
    path: PathBuf 
) -> BuckyResult<()> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    LocalChunkWriter::new(
        path.to_owned(), None, chunk, 
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker()
    ).track_path().await
}

pub async fn track_chunk_to_path(
    stack: &Stack, 
    chunk: &ChunkId, 
    content: Arc<Vec<u8>>, 
    path: &Path, 
) -> BuckyResult<()> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    LocalChunkWriter::from_path(
        path, chunk, 
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker()
    ).write(chunk, content).await
}

pub async fn download_chunk_to_path(
    stack: &Stack, 
    chunk: ChunkId, 
    config: ChunkDownloadConfig, 
    path: &Path
) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    let writer = LocalChunkWriter::from_path(
        path, &chunk, 
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker());
    let writer = Box::new(writer) as Box<dyn ChunkWriter>;
    download_chunk(stack, chunk, config, vec![writer]).await
}

pub async fn download_chunk(
    stack: &Stack, 
    chunk: ChunkId, 
    config: ChunkDownloadConfig, 
    writers: Vec<Box<dyn ChunkWriter>>
) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    let _ = stack.ndn().chunk_manager().track_chunk(&chunk).await?;
    // 默认写到cache里面去
    let task = ChunkTask::new(
        stack.to_weak(), 
        chunk, 
        Arc::new(config), 
        writers, 
        stack.ndn().root_task().download().resource().clone(),
        None
    );
    let _ = stack.ndn().root_task().download().add_task(task.clone_as_download_task())?;
    Ok(Box::new(task))
}

pub async fn download_chunk_list(
    stack: &Stack, 
    name: String, 
    chunks: &Vec<ChunkId>, 
    config: ChunkDownloadConfig, 
    writers: Vec<Box<dyn ChunkWriter>>
) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    let chunk_list = ChunkListDesc::from_chunks(chunks);
    let _ = futures::future::try_join_all(chunks.iter().map(|chunk| stack.ndn().chunk_manager().track_chunk(chunk))).await?;
    let task = ChunkListTask::new(
        stack.to_weak(), 
        name, 
        chunk_list, 
        Arc::new(config), 
        writers, 
        stack.ndn().root_task().download().resource().clone(),
        None);
    let _ = stack.ndn().root_task().download().add_task(task.clone_as_download_task())?;
    Ok(Box::new(task))
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
    config: ChunkDownloadConfig, 
    writers: Vec<Box<dyn ChunkWriter>>
) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    stack.ndn().chunk_manager().track_file(&file).await?;
    let chunk_list = ChunkListDesc::from_file(&file)?;
    let task = FileTask::new(
        stack.to_weak(), 
        file, 
        Some(chunk_list), 
        Arc::new(config), 
        writers, 
        stack.ndn().root_task().download().resource().clone(),
        None);
    let _ = stack.ndn().root_task().download().add_task(task.clone_as_download_task())?;
    Ok(Box::new(task))
}

pub async fn download_file_with_ranges(
    stack: &Stack, 
    file: File, 
    ranges: Option<Vec<Range<u64>>>, 
    config: ChunkDownloadConfig, 
    writers: Vec<Box<dyn ChunkWriterExt>>
) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    stack.ndn().chunk_manager().track_file(&file).await?;
    let chunk_list = ChunkListDesc::from_file(&file)?;
    let task = FileTask::with_ranges(
        stack.to_weak(), 
        file, 
        Some(chunk_list), 
        ranges, 
        Arc::new(config), 
        writers, 
        stack.ndn().root_task().download().resource().clone(),
        None);
    let _ = stack.ndn().root_task().download().add_task(task.clone_as_download_task())?;
    Ok(Box::new(task))
}


pub async fn download_file_to_path(
    stack: &Stack, 
    file: File, 
    config: ChunkDownloadConfig, 
    path: &Path) -> BuckyResult<Box<dyn DownloadTaskControl>> {
    let chunk_list = ChunkListDesc::from_file(&file)?;
    let writer = LocalChunkListWriter::new(
        path.to_owned(), &chunk_list, 
        stack.ndn().chunk_manager().ndc(), 
        stack.ndn().chunk_manager().tracker());
    let writer = Box::new(writer) as Box<dyn ChunkWriter>;
    download_file(stack, file, config, vec![writer]).await
}




#[async_trait::async_trait]
pub trait ChunkRangeWriter: Send + Sync {
    async fn write(&self, content: &[u8]) -> BuckyResult<()>;
}

struct ChunkRangeImpl {
    chunk: ChunkId, 
    ranges: Vec<(usize, Vec<Box<dyn ChunkRangeWriter>>)>
}

#[derive(Clone)]
pub struct ChunkRange(Arc<ChunkRangeImpl>);

impl ChunkRange {
    pub fn new(
        chunk: ChunkId, 
        ranges: Vec<(usize, Vec<Box<dyn ChunkRangeWriter>>)>) -> Self {
        Self(Arc::new(ChunkRangeImpl {
            chunk, 
            ranges
        }))
    }

    pub fn pathes(
        chunk: ChunkId, 
        ranges: Vec<(usize, Vec<PathBuf>)>) -> Self {
        
        struct PathWriter {
            path: PathBuf
        }

        #[async_trait::async_trait]
        impl ChunkRangeWriter for PathWriter {
            async fn write(&self, content: &[u8]) -> BuckyResult<()> {
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .append(true)
                    .open(self.path.as_path())
                    .await?;
                let _ = file.write(content).await?;
                Ok(())
            }
        }


        let ranges = ranges.into_iter()
            .map(|(r, pathes)| {
                let writers = pathes.into_iter().map(|path| Box::new(PathWriter {path}) as Box<dyn ChunkRangeWriter>).collect();
                (r, writers)
            }).collect();
        Self::new(chunk, ranges)
    }
    
}

impl std::fmt::Display for ChunkRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChunkRange::{{chunk:{}}}",  self.0.chunk)
    }
}

#[async_trait::async_trait]
impl ChunkWriter for ChunkRange {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn write(&self, _chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        let mut pre = 0;
        for (r, writers) in &self.0.ranges {
            let end = pre + *r;
            for w in writers {
                let _ = w.write(&content[pre..end]).await;
            }
            pre = end;
        }
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn err(&self, _: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }
}

pub struct DirTaskPathControl {
    stack: WeakStack, 
    path: PathBuf, 
    task: Box<dyn DirTaskControl>
}

impl DirTaskPathControl {
    fn stack(&self) -> Stack {
        Stack::from(&self.stack)
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn as_dir_control(&self) -> &dyn DirTaskControl {
        self.task.as_ref()
    } 

    pub fn add_chunk_pathes(&self, chunk: ChunkId, pathes: Vec<(usize, Vec<PathBuf>)>) -> BuckyResult<()> {
        let r = ChunkRange::pathes(chunk.clone(), pathes);
        self.task.add_chunk(chunk, vec![Box::new(r)])
    }

    pub fn add_file_path(&self, file: File, path: &Path) -> BuckyResult<()> {
        
        let stack = self.stack();
        let chunk_list = ChunkListDesc::from_file(&file)?;
        let writer = LocalChunkListWriter::new(
            path.to_owned(), &chunk_list, 
            stack.ndn().chunk_manager().ndc(), 
            stack.ndn().chunk_manager().tracker());
        let writer = Box::new(writer) as Box<dyn ChunkWriter>;
        
        self.task.add_file(file, vec![writer])
    }

    pub fn add_dir_path(&self, dir: DirId, path: PathBuf) -> BuckyResult<DirTaskPathControl> {
        let sub_task = self.task.add_dir(dir, vec![])?;
        Ok(Self {
            stack: self.stack.clone(), 
            path, 
            task: sub_task
        })
    }

    pub fn finish(&self) -> BuckyResult<()> {
        self.task.finish()
    }
}

pub fn download_dir_to_path(
    stack: &Stack, 
    dir: DirId, 
    config: ChunkDownloadConfig, 
    path: &Path
) -> BuckyResult<(Box<dyn DownloadTaskControl>, DirTaskPathControl)> {
    let task = DirTask::new(
        stack.to_weak(), 
        dir, 
        Arc::new(config), 
        vec![], 
        stack.ndn().root_task().download().resource().clone(),
        None);
    let _ = stack.ndn().root_task().download().add_task(task.clone_as_download_task())?;
    Ok((
        Box::new(task.clone()), 
        DirTaskPathControl {
            stack: stack.to_weak(), 
            path: path.to_owned(), 
            task: Box::new(task)
    }))
}
