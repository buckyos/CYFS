use cyfs_bdt::ChunkListTaskReader;

use async_std::io::{Read, Result};
use std::io::{Seek, SeekFrom};
use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct ChunkListTaskRangesReader {
    task_id: String,
    ranges: Vec<Range<u64>>,
    range_index: usize,
    reader: ChunkListTaskReader,
}

impl ChunkListTaskRangesReader {
    pub fn new(task_id: String, ranges: Vec<Range<u64>>, reader: ChunkListTaskReader) -> Self {
        Self {
            task_id,
            ranges,
            range_index: 0,
            reader,
        }
    }
}

impl Read for ChunkListTaskRangesReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        loop {
            if self.range_index >= self.ranges.len() {
                break Poll::Ready(Ok(0));
            }

            let mut range = self.ranges[self.range_index].clone();
            if range.is_empty() {
                self.range_index += 1;
                continue;
            }

            let pos = self.reader.seek(SeekFrom::Start(range.start))?;
            if pos != range.start {
                let msg = format!(
                    "seek reader but out of range! task={}, except={}, got={}",
                    self.task_id, range.start, pos
                );
                error!("{}", msg);
                let e = std::io::Error::new(std::io::ErrorKind::InvalidInput, msg);
                break Poll::Ready(Err(e));
            }

            let range_len = range.end - range.start;
            let except_len = std::cmp::min(range_len, buf.len() as u64);
            let sub_buf = &mut buf[..except_len as usize];

            match Pin::new(&mut self.reader).poll_read(cx, sub_buf) {
                Poll::Ready(ret) => match ret {
                    Ok(size) => {
                        assert!(size as u64 <= except_len);
                        range.start += size as u64;
                        let range_index = self.range_index;
                        self.ranges[range_index] = range;
                        break Poll::Ready(Ok(size));
                    }
                    Err(e) => {
                        break Poll::Ready(Err(e));
                    }
                },
                Poll::Pending => {
                    break Poll::Pending;
                }
            }
        }
    }
}

// 用以处理0长度的file和chunk
pub fn zero_bytes_reader() -> Box<dyn Read + Unpin + Send + Sync + 'static> {
    Box::new(async_std::io::Cursor::new(vec![]))
}