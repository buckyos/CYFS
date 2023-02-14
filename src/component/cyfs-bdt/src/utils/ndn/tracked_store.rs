use log::*;
use std::{
    sync::{Arc},
    path::{Path, PathBuf},
};
use async_std::{
    prelude::*, 
    fs::{self, OpenOptions}, 
    io::{SeekFrom, Cursor}, 
};
use cyfs_base::*;
use cyfs_util::*;
use crate::{ 
    ndn::*
};


struct StoreImpl {
    ndc: Box<dyn NamedDataCache>, 
    tracker: Box<dyn TrackerCache>, 
}


#[derive(Clone)]
pub struct TrackedChunkStore(Arc<StoreImpl>);

impl TrackedChunkStore {
    pub fn new(
        ndc: Box<dyn NamedDataCache>, 
        tracker: Box<dyn TrackerCache>, 
    ) -> Self {
        Self(Arc::new(StoreImpl { 
            ndc, 
            tracker, 
        }))
    }

    pub async fn track_chunk(&self, chunk: &ChunkId) -> BuckyResult<()> {
        let request = InsertChunkRequest {
            chunk_id: chunk.to_owned(),
            state: ChunkState::Unknown,
            ref_objects: None,
            trans_sessions: None,
            flags: 0,
        };

        self.ndc().insert_chunk(&request).await.map_err(|e| {
            error!("record file chunk to ndc error! chunk={}, {}",chunk, e);
            e
        })
    }

    pub async fn track_file(&self, file: &File) -> BuckyResult<()> {
        let file_id = file.desc().calculate_id();
        match file.body() {
            Some(body) => {
                let chunk_list = body.content().inner_chunk_list();
                match chunk_list {
                    Some(chunks) => {
                        for chunk in chunks {
                            // 先添加到chunk索引
                            let ref_obj = ChunkObjectRef {
                                object_id: file_id.to_owned(),
                                relation: ChunkObjectRelation::FileBody,
                            };
                
                            let req = InsertChunkRequest {
                                chunk_id: chunk.to_owned(),
                                state: ChunkState::Unknown,
                                ref_objects: Some(vec![ref_obj]),
                                trans_sessions: None,
                                flags: 0,
                            };
                
                            self.ndc().insert_chunk(&req).await.map_err(|e| {
                                error!("record file chunk to ndc error! file={}, chunk={}, {}", file_id, chunk, e);
                                e
                            })?;

                            info!("insert chunk of file to ndc, chunk:{}, file:{}", chunk, file_id);
                        }
                        Ok(())
                    }
                    None => Err(BuckyError::new(
                        BuckyErrorCode::NotSupport,
                        format!("file object should has chunk list: {}", file_id),
                    )),
                }
            }
            None => {
                Err(BuckyError::new(
                    BuckyErrorCode::InvalidFormat,
                    format!("file object should has body: {}", file_id),
                ))
            }
        }
    }


    pub async fn track_file_in_path(
        &self, 
        file: File, 
        path: PathBuf 
    ) -> BuckyResult<()> {
        let _ = self.track_file(&file).await?;
        TrackedChunkListWriter::new(
            path, 
            &ChunkListDesc::from_file(&file)?,  
            self.ndc(), 
            self.tracker()
        ).track_path().await
    }

    fn ndc(&self) -> &dyn NamedDataCache {
        self.0.ndc.as_ref()
    }

    fn tracker(&self) -> &dyn TrackerCache {
        self.0.tracker.as_ref()
    }

    
    async fn read_chunk_from_file(chunk: &ChunkId, path: &Path, offset: u64) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        debug!("begin read {} from file {:?}", chunk, path);
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .await
            .map_err(|e| {
                let msg = format!("open file {:?} failed for {}", path, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let actual_offset = file.seek(SeekFrom::Start(offset)).await.map_err(|e| {
            let msg = format!("seek file {:?} to offset {} failed for {}", path, offset, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if actual_offset != offset {
            let msg = format!(
                "seek file {:?} to offset {} actual offset {}",
                path, offset, actual_offset
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let mut content = Vec::with_capacity(chunk.len());
        unsafe { content.set_len(chunk.len()) };
        file.read_exact(&mut content).await.map_err(|e| {
            let msg = format!(
                "read chunk from file {:?} error, chunk={}, len={}, {}",
                path,
                chunk,
                chunk.len(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let actual_id = ChunkId::calculate(content.as_slice()).await?;

        if actual_id.eq(chunk) {
            debug!("read {} from file {:?}", chunk, path);
            Ok(Box::new(Cursor::new(content)))
        } else {
            let msg = format!("content in file {:?} not match chunk id", path);
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    async fn is_chunk_stored_in_file(&self, chunk: &ChunkId, path: &Path) -> BuckyResult<bool> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(),
            direction: Some(TrackerDirection::Store),
        };
        let ret = self.0.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            Ok(false)
        } else {
            for c in ret {
                match &c.pos {
                    TrackerPostion::File(exists) => {
                        if path.eq(Path::new(exists)) {
                            return Ok(true);
                        }
                    }
                    TrackerPostion::FileRange(fr) => {
                        if path.eq(Path::new(&fr.path)) {
                            return Ok(true);
                        }
                    }
                    _ => {}
                }
            }
            Ok(false)
        }
    }
}


#[async_trait::async_trait]
impl ChunkReader for TrackedChunkStore {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        let request = GetChunkRequest {
            chunk_id: chunk.clone(),
            flags: 0,
        };
        match self.ndc().get_chunk(&request).await {
            Ok(c) => {
                if let Some(c) = c {
                    c.state == ChunkState::Ready
                } else {
                    false
                }
            }
            Err(e) => {
                error!("got chunk state {} from database failed for {}", chunk, e);
                false
            }
        }
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Box<dyn AsyncReadWithSeek + Unpin + Send + Sync>> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(),
            direction: Some(TrackerDirection::Store),
        };
        let ret = self.tracker().get_position(&request).await?;
        if ret.len() == 0 {
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "chunk not exists",
            ))
        } else {
            for c in ret {
                let read_ret = match &c.pos {
                    //FIXME
                    TrackerPostion::File(path) => {
                        Self::read_chunk_from_file(chunk, Path::new(path), 0).await
                    }
                    TrackerPostion::FileRange(fr) => {
                        Self::read_chunk_from_file(
                            chunk,
                            Path::new(fr.path.as_str()),
                            fr.range_begin,
                        )
                        .await
                    }
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::InvalidFormat,
                        "unsupport reader",
                    )),
                };

                match read_ret {
                    Ok(reader) => {
                        return Ok(reader);
                    }, 
                    Err(e) => {
                        // 如果tracker中的pos无法正确读取，从tracker中删除这条记录
                        let _ = self
                            .0
                            .tracker
                            .remove_position(&RemoveTrackerPositionRequest {
                                id: chunk.to_string(),
                                direction: Some(TrackerDirection::Store),
                                pos: Some(c.pos.clone()),
                            })
                            .await;
                        error!(
                            "read {} from tracker position {:?} failed for {}",
                            chunk, c.pos, e
                        );
                        continue;
                    }
                }
            }

            error!("read {} from all tracker position failed", chunk);
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "chunk not exists",
            ))
        }
    }
}


struct WriterImpl {
    path: PathBuf,
    tmp_path: Option<PathBuf>,
    chunk: ChunkId,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
}

#[derive(Clone)]
pub struct TrackedChunkWriter(Arc<WriterImpl>);

impl std::fmt::Display for TrackedChunkWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrackedChunkWriter{{path:{:?}}}", self.path())
    }
}


impl TrackedChunkWriter {
    fn from_path(
        path: &Path,
        chunk: &ChunkId,
        ndc: &dyn NamedDataCache,
        tracker: &dyn TrackerCache,
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
            ndc,
            tracker,
        )
    }


    fn new(
        path: PathBuf,
        tmp_path: Option<PathBuf>,
        chunk: &ChunkId,
        ndc: &dyn NamedDataCache,
        tracker: &dyn TrackerCache,
    ) -> Self {
        Self(Arc::new(WriterImpl {
            path,
            tmp_path,
            chunk: chunk.clone(),
            ndc: ndc.clone(),
            tracker: tracker.clone(),
        }))
    }

    pub async fn track_path(&self) -> BuckyResult<()> {
        let request = UpdateChunkStateRequest {
            chunk_id: self.chunk().clone(),
            current_state: None,
            state: ChunkState::Ready,
        };
        let _ = self.0.ndc.update_chunk_state(&request).await.map_err(|e| {
            error!("{} add to tracker failed for {}", self, e);
            e
        })?;
        let request = AddTrackerPositonRequest {
            id: self.chunk().to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::File(self.path().to_str().unwrap().to_string()),
            flags: 0,
        };
        self.0.tracker.add_position(&request).await.map_err(|e| {
            error!("{} add to tracker failed for {}", self, e);
            e
        })?;

        Ok(())
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

        self.track_path().await
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
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
}

#[derive(Clone)]
pub struct TrackedChunkListWriter(Arc<ListWriterImpl>);

impl std::fmt::Display for TrackedChunkListWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TrackedChunkListWriter{{path:{:?}}}", self.path())
    }
}

impl TrackedChunkListWriter {
    fn new(
        path: PathBuf, 
        desc: &ChunkListDesc, 
        ndc: &dyn NamedDataCache, 
        tracker: &dyn TrackerCache) -> Self {
        
        Self(Arc::new(ListWriterImpl {
            path, 
            desc: desc.clone(),  
            ndc: ndc.clone(), 
            tracker: tracker.clone(),  
        }))
    }

    async fn track_chunk_index(&self, chunk: &ChunkId, index: usize) -> BuckyResult<()> {
        let offset = self.chunk_list().offset_of(index).unwrap();

        let request = UpdateChunkStateRequest {
            chunk_id: chunk.clone(),
            current_state: None,
            state: ChunkState::Ready,
        };
        let _ = self.0.ndc.update_chunk_state(&request).await.map_err(|e| {
            error!("{} add {} to tracker failed for {}", self, chunk, e);
            e
        })?;
        let request = AddTrackerPositonRequest {
            id: chunk.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::FileRange(PostionFileRange {
                path: self.path().to_str().unwrap().to_string(),
                range_begin: offset,
                range_end: offset + chunk.len() as u64,
            }),
            flags: 0,
        };
        self.0.tracker.add_position(&request).await.map_err(|e| {
            error!("{} add {} to tracker failed for {}", self, chunk, e);
            e
        })?;

        Ok(())
    }

    pub async fn track_path(&self) -> BuckyResult<()> {
        for (index, chunk) in self.chunk_list().chunks().iter().enumerate() {
            let _ = self.track_chunk_index(chunk, index).await?;
        }
        Ok(())
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

        for (index, chunk) in self.chunk_list().chunks().iter().enumerate() {
            if chunk.len() == 0 {
                continue;
            }

            let mut buffer = vec![0u8; chunk.len()];
            reader.read_exact(&mut buffer[..]).await?;

            file.write_all(&buffer[..]).await?;

            let _ = self.track_chunk_index(chunk, index).await?;
        }

        Ok(())
    }
}


impl TrackedChunkStore {
    pub async fn chunk_writer(
        &self,
        chunk: &ChunkId, 
        path: PathBuf
    ) -> BuckyResult<TrackedChunkWriter> {
        let _ = self.track_chunk(chunk).await?;
        Ok(TrackedChunkWriter::new(path, None, chunk, self.ndc(), self.tracker()))
    }

    pub async fn chunk_list_writer(
        &self,  
        chunk_list: &ChunkListDesc, 
        path: PathBuf
    ) -> BuckyResult<TrackedChunkListWriter> {
        for chunk in chunk_list.chunks() {
            let _ = self.track_chunk(chunk).await?;
        }
        Ok(TrackedChunkListWriter::new(path, chunk_list, self.ndc(), self.tracker()))
    }

    pub async fn file_writer(
        &self,
        file: &File, 
        path: PathBuf 
    ) -> BuckyResult<TrackedChunkListWriter> {
        let _ = self.track_file(file).await?;
        Ok(TrackedChunkListWriter::new(path, &ChunkListDesc::from_file(&file)?, self.ndc(), self.tracker()))
    }
}