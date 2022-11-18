use log::*;
use std::{
    sync::{Arc},
    path::{Path, PathBuf},
};
use async_std::{
    prelude::*, 
    fs::{self, OpenOptions},  
};
use cyfs_base::*;

use crate::{
    ndn::*
};


struct WriterImpl {
    path: PathBuf,
    tmp_path: Option<PathBuf>,
    chunk: ChunkId,
}

#[derive(Clone)]
pub struct LocalChunkWriter(Arc<WriterImpl>);

impl std::fmt::Display for LocalChunkWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalChunkWriter{{path:{:?}}}", self.path())
    }
}


impl LocalChunkWriter {
    pub fn from_path(
        path: &Path,
        chunk: &ChunkId,
    ) -> Self {
        let tmp_path = format!(
            "{}-{}",
            path.file_name().unwrap().to_str().unwrap(),
            bucky_time_now()
        );
        Self::new(
            path.to_owned(),
            Some(path.parent().unwrap().join(tmp_path.as_str())),
            chunk,
        )
    }


    pub fn new(
        path: PathBuf,
        tmp_path: Option<PathBuf>,
        chunk: &ChunkId,
    ) -> Self {
        Self(Arc::new(WriterImpl {
            path,
            tmp_path,
            chunk: chunk.clone(),
        }))
    }

    
    fn path(&self) -> &Path {
        self.0.path.as_path()
    }

    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }


    async fn write_inner<R: async_std::io::Read + Unpin>(&self, reader: R) -> BuckyResult<()> {
        if self.chunk().len() == 0 {
            return Ok(());
        }

        let path = self.0.tmp_path.as_ref().map(|p| p.as_path()).unwrap_or(self.path());

        let file = OpenOptions::new().create(true).write(true).open(path).await
            .map_err(|e| {
                let msg = format!("{} open file failed for {}", self, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let _ = async_std::io::copy(reader, file).await
            .map_err(|e| {
                let msg = format!(
                    "{} write chunk file failed for {}",
                    self, 
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        
            
        if self.0.tmp_path.is_some() {
            let tmp_path = self.0.tmp_path.as_ref().unwrap().as_path();
            let ret = fs::rename(tmp_path, self.path()).await;
            if ret.is_err() {
                if !self.path().exists() {
                    let msg = format!("{} rename tmp file failed for {}", self, ret.err().unwrap());
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
                }
            }
        }

        info!("{} writen chunk to file", self);

        Ok(())
    }

    pub async fn write<R: async_std::io::Read + Unpin>(&self, reader: R) -> BuckyResult<()> {
        if self.chunk().len() == 0 {
            return Ok(());
        }

        let ret = self.write_inner(reader).await;

        if self.0.tmp_path.is_some() {
            let tmp_path = self.0.tmp_path.as_ref().unwrap().as_path();
            let _ = fs::remove_file(tmp_path).await;
        }
        
        ret
    }
}


struct ListWriterImpl {
    path: PathBuf,
    desc: ChunkListDesc,
}

#[derive(Clone)]
pub struct LocalChunkListWriter(Arc<ListWriterImpl>);

impl std::fmt::Display for LocalChunkListWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalChunkListWriter{{path:{:?}}}", self.path())
    }
}

impl LocalChunkListWriter {
    pub fn from_file(
        path: PathBuf, 
        file: &File
    ) -> BuckyResult<Self> {
        Ok(Self::new(path, &ChunkListDesc::from_file(&file)?))
    }

    pub fn new(
        path: PathBuf, 
        desc: &ChunkListDesc
    ) -> Self {
        
        Self(Arc::new(ListWriterImpl {
            path, 
            desc: desc.clone(),  
        }))
    }


    fn path(&self) -> &Path {
        self.0.path.as_path()
    }

    fn chunk_list(&self) -> &ChunkListDesc {
        &self.0.desc
    }

    pub async fn write<R: async_std::io::Read + Unpin>(&self, reader: R) -> BuckyResult<()> {
        // 零长度的chunk不需要触发真正的写入操作
        if self.chunk_list().total_len() == 0 {
            return Ok(());
        }

        let mut reader = reader;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(self.path())
            .await
            .map_err(|e| {
                let msg = format!("{} open file failed for {}", self, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        // 强制设置为目标大小
        file.set_len(self.chunk_list().total_len())
            .await
            .map_err(|e| {
                let msg = format!(
                    "{} create trans data file with len {} failed for {}",
                    self,
                    self.chunk_list().total_len(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        // 强制设置为目标大小
        file.set_len(self.chunk_list().total_len()).await.map_err(|e| {
            let msg = format!(
                "{} create trans data file with len {} failed for {}",
                self, 
                self.chunk_list().total_len(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        for chunk in self.chunk_list().chunks().iter() {
            if chunk.len() == 0 {
                continue;
            }

            let mut buffer = vec![0u8; chunk.len()];
            reader.read_exact(&mut buffer[..]).await?;

            file.write_all(&buffer[..]).await?;
        }

        Ok(())
    }
}



pub fn local_chunk_writer(
    chunk: &ChunkId, 
    path: PathBuf
) -> LocalChunkWriter {
    LocalChunkWriter::new(path, None, chunk)
}

pub fn local_file_writer(
    file: &File, 
    path: PathBuf 
) -> BuckyResult<LocalChunkListWriter> {
    Ok(LocalChunkListWriter::new(path, &ChunkListDesc::from_file(&file)?))
}