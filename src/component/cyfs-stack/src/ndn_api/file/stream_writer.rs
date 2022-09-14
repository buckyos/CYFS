use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_bdt::{ChunkWriter, ChunkWriterExt, DownloadTask};

use async_std::io::{Read, Result};
use futures::future::{AbortHandle, AbortRegistration, Abortable};
use std::ops::Range;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::task::{Context, Poll, Waker};


struct FileChunkListStreamWriterImpl {
    task_id: String,

    chunk_list: Vec<Box<dyn Read + Unpin + Send + Sync + 'static>>,
    has_err: Option<std::io::Error>,
    waker: Option<Waker>,

    // 已经下载完毕的size
    ready_size: usize,
    is_end: bool,

    total_size: usize,
}

impl FileChunkListStreamWriterImpl {
    pub fn new(object_id: &ObjectId, total_size: usize) -> Self {
        static TASK_INDEX: AtomicU32 = AtomicU32::new(0);

        let task_index = TASK_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let task_id = format!("{}_{}_{}", object_id, total_size, task_index);

        debug!("new stream writer: object={}, total={}, task={}", object_id, total_size, task_id);

        Self {
            task_id,
            chunk_list: Vec::new(),
            ready_size: 0,
            is_end: false,
            has_err: None,
            waker: None,
            total_size,
        }
    }

    pub fn append(
        &mut self,
        chunk_id: &ChunkId,
        chunk: Box<dyn Read + Unpin + Send + Sync + 'static>,
        len: usize,
    ) {
        self.chunk_list.push(chunk);

        assert!(!self.is_end);
        assert!(chunk_id.len() >= len);

        self.ready_size += len;
        if self.ready_size >= self.total_size {
            self.is_end = true;
        }
        debug!(
            "chunklist append chunk: {}, len={}, is_end={}",
            self.task_id, len, self.is_end
        );
        if let Some(waker) = self.waker.take() {
            trace!("chunklist will wake on new chunk {}", self.task_id);
            waker.wake();
        }
    }

    pub fn append_buffer(&mut self, chunk_id: &ChunkId, chunk: Vec<u8>) {
        let len = chunk.len();
        let chunk = Box::new(async_std::io::Cursor::new(chunk));
        self.append(chunk_id, chunk, len)
    }

    pub fn append_shared_buffer(&mut self, chunk_id: &ChunkId, chunk: Arc<Vec<u8>>) {
        let chunk = chunk[..].to_owned();
        self.append_buffer(chunk_id, chunk)
    }

    pub fn finish(&mut self) {
        debug!(
            "stream writer finished: {}, ready={}, total={}",
            self.task_id, self.ready_size, self.total_size
        );

        // 对于零字节长度的文件，可能不会触发write chunk，而直接触发finish
        assert!(self.is_end || self.total_size == 0);

        if !self.is_end {
            self.is_end = true;
        }

        if let Some(waker) = self.waker.take() {
            trace!("chunklist will wake on finish {}", self.task_id,);
            waker.wake();
        }
    }

    pub fn error(&mut self, e: BuckyError) {
        warn!("chunk stream write error: {}, {}", self.task_id, e);

        let mut is_io_error = false;
        if let Some(e) = e.origin() {
            match e {
                BuckyOriginError::IoError(_) => {
                    is_io_error = true;
                }
                _ => {}
            }
        }

        if is_io_error {
            let e = e.into_origin().unwrap();
            match e {
                BuckyOriginError::IoError(e) => self.has_err = Some(e),
                _ => {
                    unreachable!();
                }
            }
        } else {
            let kind = e.code().into();
            let e = std::io::Error::new(kind, e);
            self.has_err = Some(e);
        }

        if let Some(waker) = self.waker.take() {
            trace!("chunklist will wake on error...");
            waker.wake();
        }
    }
}

#[derive(Clone)]
pub struct FileChunkListStreamWriter(Arc<Mutex<FileChunkListStreamWriterImpl>>);

impl FileChunkListStreamWriter {
    pub fn new(object_id: &ObjectId, total_size: usize) -> Self {
        Self(Arc::new(Mutex::new(FileChunkListStreamWriterImpl::new(
            object_id, total_size,
        ))))
    }

    pub fn task_id(&self) -> String {
        self.0.lock().unwrap().task_id.clone()
    }

    pub fn append(&self, chunk_id: &ChunkId, chunk: Box<dyn Read + Unpin + Send + Sync + 'static>) {
        debug!("append chunk {}, chunk={}", self.task_id(), chunk_id);
        self.0
            .lock()
            .unwrap()
            .append(chunk_id, chunk, chunk_id.len())
    }

    pub fn append_buffer(&self, chunk_id: &ChunkId, chunk: Vec<u8>) {
        debug!(
            "append chunk buffer {}, chunk={}, len={}",
            self.task_id(),
            chunk_id,
            chunk.len()
        );
        self.0.lock().unwrap().append_buffer(chunk_id, chunk)
    }

    pub fn append_shared_buffer(&self, chunk_id: &ChunkId, chunk: Arc<Vec<u8>>) {
        debug!(
            "append shared buffer {}, chunk={}, len={}",
            self.task_id(),
            chunk_id,
            chunk.len()
        );
        self.0.lock().unwrap().append_shared_buffer(chunk_id, chunk)
    }

    pub fn finish(&self) {
        debug!("write finished {}", self.task_id(),);
        self.0.lock().unwrap().finish()
    }

    pub fn error(&self, e: BuckyError) {
        debug!("write error {}, {}", self.task_id(), e);
        self.0.lock().unwrap().error(e)
    }

    pub fn total_size(&self) -> usize {
        self.0.lock().unwrap().total_size
    }

    pub fn remain_size(&self) -> usize {
        let inner = self.0.lock().unwrap();
        inner.total_size - inner.ready_size
    }

    pub fn into_writer(self) -> Box<dyn ChunkWriter> {
        Box::new(self) as Box<dyn ChunkWriter>
    }

    pub fn into_writer_ext(self) -> Box<dyn ChunkWriterExt> {
        Box::new(self) as Box<dyn ChunkWriterExt>
    }
}

impl std::fmt::Display for FileChunkListStreamWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let this = self.0.lock().unwrap();
        write!(f, "task: {}, ", this.task_id)?;
        write!(f, ", total_size: {}, ", this.total_size)?;
        write!(f, ", ready_size: {}, ", this.ready_size)?;
        write!(f, ", is_end: {}, ", this.is_end)?;
        write!(f, ", has_err: {:?}", this.has_err)
    }
}

#[async_trait::async_trait]
impl ChunkWriter for FileChunkListStreamWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone()) as Box<dyn ChunkWriter>
    }

    async fn write(&self, chunk_id: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        self.append_shared_buffer(chunk_id, content);
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Self::finish(&self);

        Ok(())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.0.lock().unwrap().error(BuckyError::from(err));
        Ok(())
    }
}

#[async_trait::async_trait]
impl ChunkWriterExt for FileChunkListStreamWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone()) as Box<dyn ChunkWriterExt>
    }

    async fn write(
        &self,
        chunk_id: &ChunkId,
        content: Arc<Vec<u8>>,
        range: Option<Range<u64>>,
    ) -> BuckyResult<()> {
        match range {
            Some(range) => {
                assert!(range.end <= content.len() as u64);
                assert!(range.start <= range.end);
                let content = content[range.start as usize..range.end as usize].to_owned();
                self.append_buffer(chunk_id, content);
            }
            None => {
                self.append_shared_buffer(chunk_id, content);
            }
        }

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Self::finish(&self);

        Ok(())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.0.lock().unwrap().error(BuckyError::from(err));
        Ok(())
    }
}

impl Read for FileChunkListStreamWriter {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        // trace!("chunklist poll read: cap={}", buf.len());

        let mut inner = self.0.lock().unwrap();

        // 如果已经出错了，那么直接返回
        if let Some(e) = inner.has_err.take() {
            return Poll::Ready(Err(e));
        }

        let cap = buf.len();

        let mut complete = 0;
        loop {
            if inner.chunk_list.is_empty() {
                if inner.is_end {
                    trace!("chunklist poll break with end {}", self.task_id());
                    break Poll::Ready(Ok(0));
                } else {
                    trace!("chunklist poll break with pending, list is empty {}", self.task_id());
                    // assert!(inner.waker.is_none());
                    inner.waker = Some(cx.waker().clone());
                    break Poll::Pending;
                }
            }

            let chunk = &mut inner.chunk_list[0];
            assert!(complete < cap);
            match Pin::new(chunk.as_mut()).poll_read(cx, &mut buf[complete..cap]) {
                Poll::Ready(ret) => {
                    match ret {
                        Ok(size) => {
                            if size > 0 {
                                complete += size;

                                // TODO 是否要填满buf再返回，还是一个chunk read返回了数据就立即返回？
                                // 这里先立即返回
                                trace!("chunklist poll break with data: {}, len={}", self.task_id(), complete);

                                break Poll::Ready(Ok(complete));
                            } else {
                                trace!("chunklist poll break with one chunk complete {}", self.task_id());

                                // 当前chunk读取完毕了，继续下一个
                                inner.chunk_list.remove(0);
                            }
                        }
                        Err(e) => {
                            trace!("chunklist poll break with one chunk error: {}, {}", self.task_id(), e);

                            // 出错了，终止
                            break Poll::Ready(Err(e));
                        }
                    }
                }
                Poll::Pending => {
                    if complete > 0 {
                        trace!("chunklist poll break with data: chunk pending but had pre chunk data: {}, {}", self.task_id(), complete);
                        break Poll::Ready(Ok(complete));
                    } else {
                        trace!("chunklist poll break with chunk poll_read return pending {}", self.task_id());
                        break Poll::Pending;
                    }
                }
            }
        }
    }
}

pub struct FileChunkListStreamReader {
    writer: FileChunkListStreamWriter,
    task: Box<dyn DownloadTask>,
}

impl FileChunkListStreamReader {
    pub fn new(writer: FileChunkListStreamWriter, task: Box<dyn DownloadTask>) -> Self {
        Self { writer, task }
    }
}

impl Drop for FileChunkListStreamReader {
    fn drop(&mut self) {
        debug!("stream reader dropped: {}", self.writer.task_id());

        if let Err(e) = self.task.cancel() {
            error!(
                "cancel ndn download task error! {}, {}",
                self.writer.task_id(), e,
            );
        } else {
            info!("cancel ndn download task {}, remain size={}", self.writer.task_id(), self.writer.remain_size());
        }
    }
}

impl Read for FileChunkListStreamReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        Pin::new(&mut self.as_mut().writer).poll_read(cx, buf)
    }
}

// 等待错误发生，或者完成第一个chunk后返回
struct FirstWakeupStreamWriterImpl {
    task_id: String,
    waker: Option<AbortHandle>,
    abort_registration: Option<AbortRegistration>,
    error: Option<BuckyError>,
}

#[derive(Clone)]
pub struct FirstWakeupStreamWriter(Arc<Mutex<FirstWakeupStreamWriterImpl>>);

impl FirstWakeupStreamWriter {
    pub fn new(task_id: String) -> Self {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let imp = FirstWakeupStreamWriterImpl {
            task_id,
            waker: Some(abort_handle),
            abort_registration: Some(abort_registration),
            error: None,
        };

        Self(Arc::new(Mutex::new(imp)))
    }

    pub async fn wait_and_return(
        &self,
        ret: Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) -> BuckyResult<Box<dyn Read + Unpin + Send + Sync + 'static>> {
        let abort_registration = self.0.lock().unwrap().abort_registration.take().unwrap();

        // 等待唤醒
        let future = Abortable::new(async_std::future::pending::<()>(), abort_registration);
        future.await.unwrap_err();

        if let Some(e) = self.0.lock().unwrap().error.take() {
            Err(e)
        } else {
            Ok(ret)
        }
    }

    fn try_wakeup(&self, err: Option<BuckyErrorCode>) {
        let waker = {
            let mut item = self.0.lock().unwrap();
            item.error = err.map(|code| BuckyError::from(code));
            item.waker.take()
        };

        if let Some(waker) = waker {
            debug!(
                "first wakeup reader will wake! {}, err={:?}",
                self.0.lock().unwrap().task_id,
                err
            );
            waker.abort();
        }
    }

    pub fn into_writer(self) -> Box<dyn ChunkWriter> {
        Box::new(self) as Box<dyn ChunkWriter>
    }

    pub fn into_writer_ext(self) -> Box<dyn ChunkWriterExt> {
        Box::new(self) as Box<dyn ChunkWriterExt>
    }
}

#[async_trait::async_trait]
impl ChunkWriter for FirstWakeupStreamWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone()) as Box<dyn ChunkWriter>
    }

    async fn write(&self, _chunk_id: &ChunkId, _content: Arc<Vec<u8>>) -> BuckyResult<()> {
        self.try_wakeup(None);
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.try_wakeup(None);

        Ok(())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.try_wakeup(Some(err));
        Ok(())
    }
}

#[async_trait::async_trait]
impl ChunkWriterExt for FirstWakeupStreamWriter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone()) as Box<dyn ChunkWriterExt>
    }

    async fn write(
        &self,
        _chunk_id: &ChunkId,
        _content: Arc<Vec<u8>>,
        _range: Option<Range<u64>>,
    ) -> BuckyResult<()> {
        self.try_wakeup(None);
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.try_wakeup(None);
        Ok(())
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.try_wakeup(Some(err));
        Ok(())
    }
}

impl std::fmt::Display for FirstWakeupStreamWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let this = self.0.lock().unwrap();
        write!(f, "task: {}, ", this.task_id)?;
        if let Some(e) = &this.error {
            write!(f, ", err: {}, ", e)?;
        }
        write!(f, ", waked: {}, ", this.waker.is_none())?;

        Ok(())
    }
}

pub struct ChunkWriterExtAdapter {
    writer: Box<dyn ChunkWriter>,
}

impl ChunkWriterExtAdapter {
    pub fn new(writer: Box<dyn ChunkWriter>) -> Self {
        Self { writer }
    }

    pub fn into_writer_ext(self) -> Box<dyn ChunkWriterExt> {
        Box::new(self) as Box<dyn ChunkWriterExt>
    }
}

impl Clone for ChunkWriterExtAdapter {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone_as_writer(),
        }
    }
}

impl std::fmt::Display for ChunkWriterExtAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.writer.fmt(f)
    }
}

#[async_trait::async_trait]
impl ChunkWriterExt for ChunkWriterExtAdapter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriterExt> {
        Box::new(self.clone()) as Box<dyn ChunkWriterExt>
    }

    async fn write(
        &self,
        chunk_id: &ChunkId,
        content: Arc<Vec<u8>>,
        _range: Option<Range<u64>>,
    ) -> BuckyResult<()> {
        self.writer.write(chunk_id, content).await
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.writer.finish().await
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.writer.err(err).await
    }
}

pub struct ChunkWriterAdapter {
    writer: Box<dyn ChunkWriterExt>,
    ranges: Vec<Range<u64>>,
}

impl ChunkWriterAdapter {
    pub fn new(writer: Box<dyn ChunkWriterExt>, ranges: Vec<Range<u64>>) -> Self {
        Self { writer, ranges }
    }

    pub fn into_writer(self) -> Box<dyn ChunkWriter> {
        Box::new(self) as Box<dyn ChunkWriter>
    }
}

impl Clone for ChunkWriterAdapter {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone_as_writer(),
            ranges: self.ranges.clone(),
        }
    }
}

impl std::fmt::Display for ChunkWriterAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.writer.fmt(f)
    }
}

#[async_trait::async_trait]
impl ChunkWriter for ChunkWriterAdapter {
    fn clone_as_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.clone()) as Box<dyn ChunkWriter>
    }

    async fn write(&self, chunk_id: &ChunkId, content: Arc<Vec<u8>>) -> BuckyResult<()> {
        for range in &self.ranges {
            self.writer
                .write(chunk_id, content.clone(), Some(range.clone()))
                .await?;
        }

        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        self.writer.finish().await
    }

    async fn err(&self, err: BuckyErrorCode) -> BuckyResult<()> {
        self.writer.err(err).await
    }
}

// 用以处理0长度的file和chunk
pub fn zero_bytes_reader() -> Box<dyn Read + Unpin + Send + Sync + 'static> {
    Box::new(async_std::io::Cursor::new(vec![]))
}
