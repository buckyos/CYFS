use super::super::cache::NDNDataCacheManager;
use super::stream_writer::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_bdt::{ChunkDownloadConfig, ChunkWriter, ChunkWriterExt, StackGuard};

use async_std::io::Read;
use std::ops::Range;

// 用以向远程device发起chunk/file操作
pub(crate) struct TargetDataManager {
    bdt_stack: StackGuard,
    data_cache: NDNDataCacheManager,
    target: DeviceId,
}

impl TargetDataManager {
    pub(crate) fn new(
        bdt_stack: StackGuard,
        data_cache: NDNDataCacheManager,
        target: DeviceId,
    ) -> Self {
        Self {
            bdt_stack,
            data_cache,
            target,
        }
    }

    pub fn target(&self) -> &DeviceId {
        &self.target
    }

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
            "will get_file from target: target={}, file={}, file_len={}, len={}, ranges={:?}, referer={}",
            self.target, file_id, file_obj.len(), total_size, ranges, referer
        );

        let mut config = ChunkDownloadConfig::force_stream(self.target.clone());
        let referer = referer.encode_string();
        config.referer = Some(referer);

        let resp = if let Some(ranges) = ranges {
            assert!(ranges.len() > 0);

            let (writers, waker, resp) = self
                .create_file_ext_writers(&file_id, file_obj, total_size as usize)
                .await;

            let controller = cyfs_bdt::download::download_file_with_ranges(
                &self.bdt_stack,
                file_obj.to_owned(),
                Some(ranges),
                config,
                writers,
            )
            .await?;

            let reader = FileChunkListStreamReader::new(resp, controller);
            waker.wait_and_return(Box::new(reader)).await?
        } else {
            let (writers, waker, resp) = self
                .create_file_writers(&file_id, file_obj, total_size as usize)
                .await;

            let controller = cyfs_bdt::download::download_file(
                &self.bdt_stack,
                file_obj.to_owned(),
                config,
                writers,
            )
            .await?;

            let reader = FileChunkListStreamReader::new(resp, controller);
            waker.wait_and_return(Box::new(reader)).await?
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
            "will get_chunk from target: target={}, chunk={}, len={}, ranges={:?}, referer={}",
            self.target, chunk_id, total_size, ranges, referer
        );

        let mut config = ChunkDownloadConfig::force_stream(self.target.clone());
        let referer = referer.encode_string();
        config.referer = Some(referer);

        let (writers, waker, resp) = self
            .create_chunk_writers(&chunk_id, ranges, total_size)
            .await;

        let controller = cyfs_bdt::download::download_chunk(
            &self.bdt_stack,
            chunk_id.to_owned(),
            config,
            writers,
        )
        .await?;

        let reader = FileChunkListStreamReader::new(resp, controller);
        let resp = waker.wait_and_return(Box::new(reader)).await?;
        Ok((resp, total_size as u64))
    }

    async fn create_file_writers(
        &self,
        file_id: &ObjectId,
        file: &File,
        total_size: usize,
    ) -> (
        Vec<Box<dyn ChunkWriter>>,
        FirstWakeupStreamWriter,
        FileChunkListStreamWriter,
    ) {
        let writer = FileChunkListStreamWriter::new(file_id, total_size);

        let mut writer_list = vec![writer.clone().into_writer()];

        // 本地缓存
        if let Ok(Some(writer)) = self.data_cache.gen_file_writer(file_id, file).await {
            writer_list.push(writer);
        }

        // 增加返回短路器
        let waker = FirstWakeupStreamWriter::new(writer.task_id());
        writer_list.push(waker.clone().into_writer());

        (writer_list, waker, writer)
    }

    async fn create_file_ext_writers(
        &self,
        file_id: &ObjectId,
        file: &File,
        total_size: usize,
    ) -> (
        Vec<Box<dyn ChunkWriterExt>>,
        FirstWakeupStreamWriter,
        FileChunkListStreamWriter,
    ) {
        let writer = FileChunkListStreamWriter::new(file_id, total_size);

        let mut writer_list = vec![writer.clone().into_writer_ext()];

        // 本地缓存
        if let Ok(Some(writer)) = self.data_cache.gen_file_writer(file_id, file).await {
            writer_list.push(ChunkWriterExtAdapter::new(writer).into_writer_ext());
        }

        // 增加返回短路器
        let waker = FirstWakeupStreamWriter::new(writer.task_id());
        writer_list.push(waker.clone().into_writer_ext());

        (writer_list, waker, writer)
    }

    async fn create_chunk_writers(
        &self,
        chunk_id: &ChunkId,
        ranges: Option<Vec<Range<u64>>>,
        total_size: usize,
    ) -> (
        Vec<Box<dyn ChunkWriter>>,
        FirstWakeupStreamWriter,
        FileChunkListStreamWriter,
    ) {
        let writer = FileChunkListStreamWriter::new(&chunk_id.object_id(), total_size);
        let ret_writer = match ranges {
            Some(ranges) => {
                ChunkWriterAdapter::new(writer.clone().into_writer_ext(), ranges).into_writer()
            }
            None => writer.clone().into_writer(),
        };

        let mut writer_list = vec![ret_writer];

        // 本地缓存
        if let Ok(Some(writer)) = self.data_cache.gen_chunk_writer(chunk_id).await {
            writer_list.push(writer);
        }

        // 增加返回短路器
        let waker = FirstWakeupStreamWriter::new(writer.task_id());
        writer_list.push(waker.clone().into_writer());

        (writer_list, waker, writer)
    }

    /*
    async fn create_chunk_writers_ext(
        &self,
        chunk_id: &ChunkId,
        total_size: usize,
    ) -> (
        Vec<Box<dyn ChunkWriterExt>>,
        FirstWakeupStreamWriter,
        Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) {
        let writer = FileChunkListStreamWriter::new(chunk_id.object_id(), total_size);

        let mut writer_list = vec![writer.clone().into_writer_ext()];

        // 本地缓存
        if let Ok(Some(writer)) = self.data_cache.gen_chunk_writer(chunk_id).await {
            writer_list.push(ChunkWriterExtAdapter::new(writer).into_writer_ext());
        }

        // 增加返回短路器
        let waker = FirstWakeupStreamWriter::new(chunk_id.object_id());
        writer_list.push(waker.clone().into_writer_ext());

        (writer_list, waker, Box::new(writer))
    }
    */

    /*
    pub async fn get_chunk_buffer(
        &self,
        chunk_id: &ChunkId,
        referer: &BdtDataRefererInfo,
    ) -> BuckyResult<Vec<u8>> {
        let mut reader = self.get_chunk(chunk_id, referer).await?;

        use async_std::io::ReadExt;

        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.map_err(|e| {
            let msg = format!("read chunk data failed! chunk={} {}", chunk_id, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        Ok(buf)
    }
    */
}
