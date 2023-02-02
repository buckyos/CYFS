use super::super::context::TransContextHolder;
use super::cache::ChunkListCacheReader;
use super::stream_reader::*;
use crate::ndn::TaskGroupHelper;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;

use async_std::io::Read;
use futures::AsyncReadExt;
use std::ops::Range;

// 用以向远程device发起chunk/file操作
pub(crate) struct TargetDataManager {
    bdt_stack: StackGuard,
    chunk_manager: ChunkManagerRef,
    context: TransContextHolder,
}

impl TargetDataManager {
    pub(crate) fn new(
        bdt_stack: StackGuard,
        chunk_manager: ChunkManagerRef,
        context: TransContextHolder,
    ) -> Self {
        Self {
            bdt_stack,
            chunk_manager,
            context,
        }
    }

    pub fn context(&self) -> String {
        self.context.debug_string()
    }

    pub async fn get_file(
        &self,
        source: &RequestSourceInfo,
        file_obj: &File,
        group: Option<&str>,
        ranges: Option<Vec<Range<u64>>>,
    ) -> BuckyResult<(
        Box<dyn Read + Unpin + Send + Sync + 'static>,
        u64,
        Option<String>,
    )> {
        let file_id = file_obj.desc().calculate_id();

        let total_size = match ranges {
            Some(ref ranges) => RangeHelper::sum(ranges),
            None => file_obj.len(),
        };

        if total_size == 0 {
            warn!(
                "zero length get_file request will return directly! file={}, file_len={}, ranges={:?},",
                file_id, file_obj.len(), ranges,
            );
            return Ok((zero_bytes_reader(), 0, None));
        }

        info!(
            "will get file data from target: {:?}, file={}, file_len={}, len={}, ranges={:?}",
            self.context.debug_string(),
            file_id,
            file_obj.len(),
            total_size,
            ranges
        );

        let group = TaskGroupHelper::new_opt_with_dec(&source.dec, group);

        let (id, reader) = cyfs_bdt::download_file(
            &self.bdt_stack,
            file_obj.to_owned(),
            group,
            self.context.clone(),
        )
        .await
        .map_err(|e| {
            error!("download file error! file={}, {}", file_id, e);
            e
        })?;

        let reader = ChunkListCacheReader::new(
            self.chunk_manager.clone(),
            file_id.to_string(),
            Box::new(reader),
        );

        let resp = if let Some(ranges) = ranges {
            assert!(ranges.len() > 0);

            let reader =
                ChunkListTaskRangesReader::new(file_id.to_string(), ranges, Box::new(reader));
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        } else {
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        };

        let resp = Self::wait_read_and_return(resp).await?;

        Ok((resp, total_size, Some(id)))
    }

    // 获取单个chunk
    pub async fn get_chunk(
        &self,
        source: &RequestSourceInfo,
        chunk_id: &ChunkId,
        group: Option<&str>,
        ranges: Option<Vec<Range<u64>>>,
    ) -> BuckyResult<(
        Box<dyn Read + Unpin + Send + Sync + 'static>,
        u64,
        Option<String>,
    )> {
        let total_size = match ranges {
            Some(ref ranges) => RangeHelper::sum(ranges) as usize,
            None => chunk_id.len(),
        };

        if total_size == 0 {
            warn!(
                "zero length get_chunk request will return directly! file={}",
                chunk_id
            );
            return Ok((zero_bytes_reader(), 0, None));
        }

        info!(
            "will get chunk data from target: {}, chunk={}, len={}, ranges={:?}",
            self.context.debug_string(),
            chunk_id,
            total_size,
            ranges
        );

        let group = TaskGroupHelper::new_opt_with_dec(&source.dec, group);

        let (id, reader) = cyfs_bdt::download_chunk(
            &self.bdt_stack,
            chunk_id.clone(),
            group,
            self.context.clone(),
        )
        .await
        .map_err(|e| {
            error!("download chunk error! chunk={}, {}", chunk_id, e);
            e
        })?;

        let reader = ChunkListCacheReader::new(
            self.chunk_manager.clone(),
            chunk_id.to_string(),
            Box::new(reader),
        );

        let resp = if let Some(ranges) = ranges {
            assert!(ranges.len() > 0);

            let reader =
                ChunkListTaskRangesReader::new(chunk_id.to_string(), ranges, Box::new(reader));
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        } else {
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        };

        let resp = Self::wait_read_and_return(resp).await?;

        Ok((resp, total_size as u64, Some(id)))
    }

    async fn wait_read_and_return(mut resp: Box<dyn Read + Unpin + Send + Sync + 'static>,) -> BuckyResult<Box<dyn Read + Unpin + Send + Sync + 'static>> {
        let mut buf = vec![0; 1];
        resp.read_exact(&mut buf).await.map_err(|e| {
            BuckyError::from(e)
        })?;

        let cursor = async_std::io::Cursor::new(buf);
        Ok(Box::new(cursor.chain(resp)))
    }
}
