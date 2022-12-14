use super::stream_reader::*;
use cyfs_base::*;
use cyfs_bdt::{SingleDownloadContext, StackGuard};
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;

use async_std::io::Read;
use std::ops::Range;

// 用以向远程device发起chunk/file操作
pub(crate) struct TargetDataManager {
    bdt_stack: StackGuard,
    chunk_manager: ChunkManagerRef,
    target: Vec<DeviceId>,
}

impl TargetDataManager {
    pub(crate) fn new(
        bdt_stack: StackGuard,
        chunk_manager: ChunkManagerRef,
        target: DeviceId,
    ) -> Self {
        Self {
            bdt_stack,
            chunk_manager,
            target: vec![target],
        }
    }

    pub fn target(&self) -> &[DeviceId] {
        &self.target
    }

    /*
    fn new_chunk_manager_writer(&self) -> Box<dyn ChunkWriter> {
        let writer = ChunkManagerWriter::new(
            self.chunk_manager.clone(),
            self.bdt_stack.ndn().chunk_manager().ndc().clone(),
            self.bdt_stack.ndn().chunk_manager().tracker().clone(),
        );

        Box::new(writer)
    }
    */

    pub async fn get_file(
        &self,
        file_obj: &File,
        ranges: Option<Vec<Range<u64>>>,
        referer: &BdtDataRefererInfo,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
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
            return Ok((zero_bytes_reader(), 0));
        }

        info!(
            "will get file data from target: target={:?}, file={}, file_len={}, len={}, ranges={:?}, referer={}",
            self.target, file_id, file_obj.len(), total_size, ranges, referer
        );

        let context = SingleDownloadContext::id_streams(
            &self.bdt_stack,
            referer.encode_string(),
            &self.target,
        )
        .await?;

        let (id, reader) =
            cyfs_bdt::download_file(&self.bdt_stack, file_obj.to_owned(), None, context)
                .await
                .map_err(|e| {
                    error!("download file error! file={}, {}", file_id, e);
                    e
                })?;

        let resp = if let Some(ranges) = ranges {
            assert!(ranges.len() > 0);

            let reader = ChunkListTaskTangesReader::new(file_id.to_string(), ranges, reader);
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        } else {
            Box::new(reader) as Box<dyn Read + Unpin + Send + Sync + 'static>
        };

        Ok((resp, total_size))
    }

    // 获取单个chunk
    pub async fn get_chunk(
        &self,
        chunk_id: &ChunkId,
        ranges: Option<Vec<Range<u64>>>,
        referer: &BdtDataRefererInfo,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
        let total_size = match ranges {
            Some(ref ranges) => RangeHelper::sum(ranges) as usize,
            None => chunk_id.len(),
        };

        if total_size == 0 {
            warn!(
                "zero length get_chunk request will return directly! file={}",
                chunk_id
            );
            return Ok((zero_bytes_reader(), 0));
        }

        info!(
            "will get chunk data from target: target={:?}, chunk={}, len={}, ranges={:?}, referer={}",
            self.target, chunk_id, total_size, ranges, referer
        );

        let context = SingleDownloadContext::id_streams(
            &self.bdt_stack,
            referer.encode_string(),
            &self.target,
        )
        .await?;

        let (_id, reader) =
            cyfs_bdt::download_chunk(&self.bdt_stack, chunk_id.clone(), None, context)
                .await
                .map_err(|e| {
                    error!("download chunk error! chunk={}, {}", chunk_id, e);
                    e
                })?;

        Ok((Box::new(reader), total_size as u64))
    }
}
