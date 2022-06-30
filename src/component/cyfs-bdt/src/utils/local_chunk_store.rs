
use log::*;
use async_trait::async_trait;
use std::{
    collections::BTreeSet, 
    sync::{Arc, RwLock},
    path::{Path, PathBuf},
    ops::Range
};
use async_std::{
    prelude::*, 
    fs::{self, OpenOptions}, 
    io::{SeekFrom}, 
    channel,
    task
};
use cyfs_base::*;
use cyfs_util::*;
use crate::{
    types::*, 
    ndn::{ChunkListDesc, ChunkWriter,ChunkWriterExt, ChunkReader}
};

struct ReaderImpl {
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>, 
}

#[derive(Clone)]
pub struct LocalChunkReader(Arc<ReaderImpl>);

impl LocalChunkReader {
    pub fn new(
        ndc: &dyn NamedDataCache, 
        tracker: &dyn TrackerCache
    ) -> Self {
        Self(Arc::new(ReaderImpl {
            ndc: ndc.clone(), 
            tracker: tracker.clone()
        }))
    }

    async fn read_chunk_from_file(chunk: &ChunkId, path: &Path, offset: u64) -> BuckyResult<Vec<u8>> {
        debug!("begin read {} from file {:?}", chunk, path);
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .await
            .map_err(|e| {
                let msg = format!(
                    "open file {:?} failed for {}",
                    path, 
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;
        
        let actual_offset = file.seek(SeekFrom::Start(offset)).await.map_err(|e| {
            let msg = format!(
                "seek file {:?} to offset {} failed for {}", 
                path, 
                offset,
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if actual_offset != offset {
            let msg = format!(
                "seek file {:?} to offset {} actual offset {}", 
                path, 
                offset,
                actual_offset
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let mut content = vec![0u8; chunk.len()];

        let read = file.read(content.as_mut_slice()).await?;

        if read != content.len() {
            let msg = format!(
                "read {} bytes from file {:?} but chunk len is {}",
                read,
                path, 
                content.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let actual_id = ChunkId::calculate(content.as_slice()).await?;

        if actual_id.eq(chunk) {
            debug!("read {} from file {:?}", chunk, path);
            Ok(content)
        } else {
            let msg = format!("content in file {:?} not match chunk id", path);
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    async fn is_chunk_stored_in_file(&self, chunk: &ChunkId, path: &Path) -> BuckyResult<bool> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(), 
            direction: Some(TrackerDirection::Store)
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
                    }, 
                    TrackerPostion::FileRange(fr) => {
                        if path.eq(Path::new(&fr.path)) {
                            return Ok(true);
                        }
                    }, 
                    _ => {

                    }
                }
            }
            Ok(false)
        }
    } 
}

#[async_trait]
impl ChunkReader for LocalChunkReader {
    fn clone_as_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.clone())
    }

    async fn exists(&self, chunk: &ChunkId) -> bool {
        let request = GetChunkRequest {
            chunk_id: chunk.clone(),
            flags: 0,
        };
        match self.0.ndc.get_chunk(&request).await {
            Ok(c) => {
                if let Some(c) = c {
                    c.state == ChunkState::Ready
                } else {
                    false
                }
            }, 
            Err(e) => {
                error!("got chunk state {} from database failed for {}", chunk, e);
                false
            }
        }
    }

    async fn get(&self, chunk: &ChunkId) -> BuckyResult<Arc<Vec<u8>>> {
        let request = GetTrackerPositionRequest {
            id: chunk.to_string(), 
            direction: Some(TrackerDirection::Store)
        };
        let ret = self.0.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "chunk not exists"))
        } else {
            for c in ret {
                let read_ret = match &c.pos {
                    //FIXME
                    TrackerPostion::File(path) => {
                        Self::read_chunk_from_file(chunk, Path::new(path), 0).await
                    }, 
                    TrackerPostion::FileRange(fr) => {
                        Self::read_chunk_from_file(chunk, Path::new(fr.path.as_str()), fr.range_begin).await
                    }, 
                    _ => {
                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, "unsupport reader"))
                    }
                };
                
                match read_ret {
                    Ok(content) => {
                        return Ok(Arc::new(content));
                    }, 
                    Err(e) => {
                        // 如果tracker中的pos无法正确读取，从tracker中删除这条记录
                        let _ = self.0.tracker.remove_position(&RemoveTrackerPositionRequest {
                            id: chunk.to_string(), 
                            direction: Some(TrackerDirection::Store), 
                            pos: Some(c.pos.clone())
                        }).await;
                        error!("read {} from tracker position {:?} failed for {}", chunk, c.pos, e);
                        continue;
                    }
                }
            }

            error!("read {} from all tracker position failed", chunk);
            Err(BuckyError::new(BuckyErrorCode::NotFound, "chunk not exists"))
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
pub struct LocalChunkWriter(Arc<WriterImpl>);

impl std::fmt::Display for LocalChunkWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalChunkWriter{{chunk:{}, path:{:?}}}", self.chunk(), self.path())
    }
}

impl LocalChunkWriter {
    pub fn from_path(
        path: &Path, 
        chunk: &ChunkId, 
        ndc: &dyn NamedDataCache, 
        tracker: &dyn TrackerCache
    ) -> Self {
        let tmp_path = format!("{}-{}", path.file_name().unwrap().to_str().unwrap(), bucky_time_now());
        Self::new(
            path.to_owned(), 
            Some(path.parent().unwrap().join(tmp_path.as_str())), 
            chunk, 
            ndc, 
            tracker
        )
    }

    pub async fn track_path(
        &self
    ) -> BuckyResult<()> {
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

    pub fn new(
        path: PathBuf, 
        tmp_path: Option<PathBuf>,  
        chunk: &ChunkId, 
        ndc: &dyn NamedDataCache, 
        tracker: &dyn TrackerCache) -> Self {
        Self(Arc::new(WriterImpl {
            path, 
            tmp_path, 
            chunk: chunk.clone(),  
            ndc: ndc.clone(), 
            tracker: tracker.clone()
        }))
    }

    async fn write_inner(&self, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        if self.chunk().len() == 0 {
            return Ok(());
        }

        let path = self.0.tmp_path.as_ref().map(|p| p.as_path()).unwrap_or(self.path());

        let mut file = OpenOptions::new().create(true).write(true).open(path).await
            .map_err(|e| {
                let msg = format!(
                    "{} open file failed for {}",
                    self,
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let _ = file.write(content.as_slice()).await
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
                    let msg = format!(
                        "{} rename tmp file failed for {}",
                        self, 
                        ret.err().unwrap()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
                }
            }
        }

        info!(
            "{} writen chunk to file",
            self
        );

        self.track_path().await
    }

    fn path(&self) -> &Path {
        self.0.path.as_path()
    }

    fn chunk(&self) -> &ChunkId {
        &self.0.chunk
    }
}


#[async_trait]
impl ChunkWriter for LocalChunkWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn err(&self, _e: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }
    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        assert!(chunk.eq(self.chunk()));

        if chunk.len() == 0 {
            return Ok(());
        }

        let ret = self.write_inner(content).await;

        if self.0.tmp_path.is_some() {
            let tmp_path = self.0.tmp_path.as_ref().unwrap().as_path();
            let _ = fs::remove_file(tmp_path).await;
        }
        
        ret
    }

    async fn finish(&self) -> BuckyResult<()> {
        // do nothing
        Ok(())
    }
}


struct WriteTaskImpl {
    chunk: ChunkId, 
    content: Arc<Vec<u8>>, 
    waiter: StateWaiter
}


enum WriteTask {
    Write(WriteTaskImpl), 
    Finish
}


struct WrittingState {
    written: BTreeSet<ChunkId>, 
    task_sender: channel::Sender<WriteTask>, 
    waiter: StateWaiter
} 

enum WriteState {
    Writting(WrittingState), 
    Finished
}

struct ListWriterImpl {
    path: PathBuf, 
    desc: ChunkListDesc, 
    ndc: Box<dyn NamedDataCache>, 
    tracker: Box<dyn TrackerCache>,
    state: RwLock<WriteState>
}

#[derive(Clone)]
pub struct LocalChunkListWriter(Arc<ListWriterImpl>);

impl std::fmt::Display for LocalChunkListWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalChunkListWriter{{path:{:?}}}", self.path())
    }
}

impl LocalChunkListWriter {
    pub fn new(
        path: PathBuf, 
        desc: &ChunkListDesc, 
        ndc: &dyn NamedDataCache, 
        tracker: &dyn TrackerCache) -> Self {
        //FIXME: 如果下载速度高于磁盘速度，这里就会出问题
        let (task_sender, task_recver) = channel::bounded(100);
        let writer = Self(Arc::new(ListWriterImpl {
            path, 
            desc: desc.clone(),  
            ndc: ndc.clone(), 
            tracker: tracker.clone(),  
            state: RwLock::new(WriteState::Writting(WrittingState {
                written: BTreeSet::new(), 
                task_sender, 
                waiter: StateWaiter::new()
            })), 
        }));

        {
            let writer = writer.clone();
            //FIXME: stop it
            task::spawn(async move {
                writer.start(task_recver).await
            });
        }
        writer
    }


    async fn track_chunk_index(
        &self, 
        chunk: &ChunkId, 
        index: usize
    ) -> BuckyResult<()> {
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

    async fn start(&self, task_recver: channel::Receiver<WriteTask>) -> BuckyResult<()> {
        loop {
            match task_recver.recv().await {
                Ok(task) => {
                    match task {
                        WriteTask::Write(task) => {
                            let _ = self.write_chunk(&task.chunk, task.content.as_slice()).await;
                            task.waiter.wake();
                        }, 
                        WriteTask::Finish => {
                            info!("{} finished", self);
                            let waiter = {
                                let state = &mut *self.0.state.write().unwrap();
                                match state {
                                    WriteState::Writting(writting) => {
                                        let mut waiter = StateWaiter::new();
                                        writting.waiter.transfer_into(&mut waiter);
                                        *state = WriteState::Finished;
                                        waiter
                                    }, 
                                    WriteState::Finished => unreachable!()
                                }
                            };

                            // 这里兼容空文件
                            if self.chunk_list().chunks().len() == 0 {
                                let _ = OpenOptions::new()
                                    .create(true)
                                    .write(true)
                                    .open(self.path())
                                    .await
                                    .map_err(|e| {
                                        let msg = format!(
                                            "{} open file failed for {}",
                                            self,
                                            e
                                        );
                                        error!("{}", msg);
                                        BuckyError::new(BuckyErrorCode::IoError, msg)
                                    });
                            }

                            waiter.wake();
                            break;
                        }
                    }
                }, 
                Err(e) => {
                    error!("{} break write loop for {}", self, e);
                    break;
                }
            }
        }
        Ok(())
    }


    async fn write_chunk_index(
        &self, 
        chunk: &ChunkId, 
        content: &[u8], 
        index: usize, 
        file: &mut async_std::fs::File
    ) -> BuckyResult<()> {
        info!("{} will write {} to file", self, index);
        let offset = self.chunk_list().offset_of(index).unwrap();
        let actual_offset = file.seek(SeekFrom::Start(offset)).await.map_err(|e| {
            let msg = format!(
                "{} seek file to offset {} failed for {}",
                self,
                offset,
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if actual_offset != offset {
            let msg = format!(
                "{} seek file to offset {} actual offset {}",
                self,
                offset,
                actual_offset
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let _ = file.write(content).await
            .map_err(|e| {
                let msg = format!(
                    "{} write chunk {} file failed for {}",
                    self, 
                    index,
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info!(
            "{} writen chunk {} to file",
            self, 
            index
        );

        self.track_chunk_index(chunk, index).await
    }

    async fn write_chunk(&self, chunk: &ChunkId, content: &[u8]) -> BuckyResult<()> {
        // 零长度的chunk不需要触发真正的写入操作
        if chunk.len() == 0 {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(self.path())
            .await
            .map_err(|e| {
                let msg = format!(
                    "{} open file failed for {}",
                    self,
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

        let chunk_indies = self.chunk_list().index_of(chunk).unwrap();

        let mut has_err = None;

        for chunk_index in chunk_indies {
            match self.write_chunk_index(chunk, content, *chunk_index, &mut file).await {
                Ok(_) => continue, 
                Err(err) => {
                    has_err = Some(err);
                    break;
                }
            }
        }
        
        let time = std::time::Instant::now();
        info!("will flush file after write chunk {} : {}", chunk, self);

        file.flush().await.map_err(|e| {
            let msg = format!(
                "flush file {} error: {}",
                self,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if has_err.is_none() {
            info!("write chunk {}, to file success! {}, during={}", chunk, self, time.elapsed().as_millis());
            Ok(())
        } else {
            Err(has_err.unwrap())
        }
    }
}

#[async_trait]
impl ChunkWriter for LocalChunkListWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone())
    }

    async fn err(&self, _e: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

<<<<<<< ours
    async fn redirect(&self, _redirect_node: &DeviceId, _redirect_referer: &String) -> BuckyResult<()> {
=======
    async fn redirect(&self, _redirect_node: &DeviceId) -> BuckyResult<()> {
>>>>>>> theirs
        Ok(())
    }

    async fn write(
        &self, 
        chunk: &ChunkId, 
        content: Arc<Vec<u8>>) -> BuckyResult<()> {
        
        let task_sender = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                WriteState::Writting(writting) => {
                    if writting.written.insert(chunk.clone()) {
                        Ok(Some(writting.task_sender.clone()))
                    } else {
                        Ok(None)
                    }  
                } 
                WriteState::Finished => Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} finished", self)))
            }
        }?;

        if let Some(task_sender) = task_sender {
            let mut waiter = StateWaiter::new();
            let notifier = waiter.new_waiter();

            let task = WriteTask::Write(WriteTaskImpl {
                chunk: chunk.clone(), 
                content, 
                waiter});
            
            let _ = task_sender.send(task).await
                .map_err(|e| BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting {}", self, e)))?;
            
            StateWaiter::wait(notifier, || ()).await;

            debug!("chunk file writer notified to return, writer:{}, chunk:{}", self, chunk);
        }

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {

        let task = WriteTask::Finish;

        let (task_sender, waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                WriteState::Writting(writting) => {
                    if writting.waiter.len() > 0 {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} has called finished", self)))
                    } else {
                        Ok((writting.task_sender.clone(), writting.waiter.new_waiter()))
                    }
                }
                WriteState::Finished => Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting", self)))
            }
        }?;
        
        let _ = task_sender.send(task).await
            .map_err(|e| BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting {}", self, e)))?;

        let _ = StateWaiter::wait(waiter, || ()).await;

        Ok(())
    }
}


#[async_trait]
impl ChunkWriterExt for LocalChunkListWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone())
    }

    async fn err(&self, _e: BuckyErrorCode) -> BuckyResult<()> {
        Ok(())
    }

    async fn write(&self, chunk: &ChunkId, content: Arc<Vec<u8>>, _range: Option<Range<u64>>) -> BuckyResult<()> {
        
        let task_sender = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                WriteState::Writting(writting) => {
                    if writting.written.insert(chunk.clone()) {
                        Ok(Some(writting.task_sender.clone()))
                    } else {
                        Ok(None)
                    }  
                } 
                WriteState::Finished => Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} finished", self)))
            }
        }?;

        if let Some(task_sender) = task_sender {
            let mut waiter = StateWaiter::new();
            let notifier = waiter.new_waiter();

            let task = WriteTask::Write(WriteTaskImpl {
                chunk: chunk.clone(), 
                content, 
                waiter});
            
            let _ = task_sender.send(task).await
                .map_err(|e| BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting {}", self, e)))?;
            
            StateWaiter::wait(notifier, || ()).await;

            debug!("chunk file writer notified to return, writer:{}, chunk:{}", self, chunk);
        }

        Ok(())
    }

<<<<<<< ours
    async fn redirect(&self, _redirect_node: &DeviceId, _: &String) -> BuckyResult<()> {
=======
    async fn redirect(&self, _redirect_node: &DeviceId) -> BuckyResult<()> {
>>>>>>> theirs
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {

        let task = WriteTask::Finish;

        let (task_sender, waiter) = {
            let state = &mut *self.0.state.write().unwrap();
            match state {
                WriteState::Writting(writting) => {
                    if writting.waiter.len() > 0 {
                        Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} has called finished", self)))
                    } else {
                        Ok((writting.task_sender.clone(), writting.waiter.new_waiter()))
                    }
                }
                WriteState::Finished => Err(BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting", self)))
            }
        }?;
        
        let _ = task_sender.send(task).await
            .map_err(|e| BuckyError::new(BuckyErrorCode::ErrorState, format!("{} not writting {}", self, e)))?;

        let _ = StateWaiter::wait(waiter, || ()).await;

        Ok(())
    }
}

