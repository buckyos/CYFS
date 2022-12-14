use cyfs_base::*;
use cyfs_debug::Mutex;
use super::super::cache::ChunkWriter;

use async_std::io::{Read, Result};
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

    pub fn append_shared_buffer(&mut self, chunk_id: &ChunkId, chunk: Vec<u8>) {
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

    pub fn append_shared_buffer(&self, chunk_id: &ChunkId, chunk: Vec<u8>) {
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
    async fn write(&self, chunk_id: &ChunkId, content: &[u8]) -> BuckyResult<()> {
        self.append_shared_buffer(chunk_id, content.to_owned());
        Ok(())
    }

    async fn finish(&self) -> BuckyResult<()> {
        Self::finish(&self);

        Ok(())
    }

    async fn err(&self, err: &BuckyError) -> BuckyResult<()> {
        self.0.lock().unwrap().error(err.to_owned());
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
                    trace!("chunklist poll break with end {}", inner.task_id);
                    break Poll::Ready(Ok(0));
                } else {
                    trace!("chunklist poll break with pending, list is empty {}", inner.task_id);
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
                                trace!("chunklist poll break with data: {}, len={}", inner.task_id, complete);

                                break Poll::Ready(Ok(complete));
                            } else {
                                trace!("chunklist poll break with one chunk complete {}", inner.task_id);

                                // 当前chunk读取完毕了，继续下一个
                                inner.chunk_list.remove(0);
                            }
                        }
                        Err(e) => {
                            trace!("chunklist poll break with one chunk error: {}, {}", inner.task_id, e);

                            // 出错了，终止
                            break Poll::Ready(Err(e));
                        }
                    }
                }
                Poll::Pending => {
                    if complete > 0 {
                        trace!("chunklist poll break with data: chunk pending but had pre chunk data: {}, {}", inner.task_id, complete);
                        break Poll::Ready(Ok(complete));
                    } else {
                        trace!("chunklist poll break with chunk poll_read return pending {}", inner.task_id);
                        break Poll::Pending;
                    }
                }
            }
        }
    }
}
