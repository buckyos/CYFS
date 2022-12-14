use super::reader::ChunkStoreReader;
use super::stream_writer::FileChunkListStreamWriter;
use cyfs_base::*;
use cyfs_bdt::ChunkReader;
use cyfs_chunk_cache::{ChunkManagerRef, ChunkType};
use cyfs_chunk_lib::{Chunk, ChunkReadWithRanges};
use cyfs_lib::*;
use cyfs_util::AsyncReadWithSeekAdapter;

use async_std::io::{Cursor, Read};
use std::convert::TryFrom;
use std::ops::Range;

pub(crate) struct LocalDataManager {
    chunk_manager: ChunkManagerRef,
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,

    reader: ChunkStoreReader,
}

impl LocalDataManager {
    pub(crate) fn new(
        chunk_manager: ChunkManagerRef,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
    ) -> Self {
        let reader = ChunkStoreReader::new(chunk_manager.clone(), ndc.clone(), tracker.clone());
        Self {
            chunk_manager,
            ndc,
            tracker,

            reader,
        }
    }

    pub async fn get_file(
        &self,
        file_obj: &File,
        ranges: Option<Vec<Range<u64>>>,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
        let total_size = file_obj.desc().content().len() as usize;
        let file_id = file_obj.desc().object_id();

        info!(
            "will local get file: file={}, size={}, range={:?}",
            file_id, total_size, ranges
        );

        match file_obj.body() {
            Some(body) => match body.content().chunk_list() {
                ChunkList::ChunkInList(list) => match ranges {
                    Some(ranges) => {
                        self.get_chunks_with_range(&file_id, total_size, list, ranges)
                            .await
                    }
                    None => self.get_chunks(&file_id, total_size, list).await,
                },
                ChunkList::ChunkInBundle(bundle) => match ranges {
                    Some(ranges) => {
                        self.get_chunks_with_range(
                            &file_id,
                            total_size,
                            bundle.chunk_list(),
                            ranges,
                        )
                        .await
                    }
                    None => {
                        self.get_chunks(&file_id, total_size, bundle.chunk_list())
                            .await
                    }
                },
                ChunkList::ChunkInFile(id) => {
                    let msg = format!(
                        "chunk in file not support yet! file={}, chunk file={}",
                        file_id, id
                    );
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
                }
            },
            None => {
                if total_size == 0 {
                    warn!("file has not body! file={}", file_id);
                    let reader = FileChunkListStreamWriter::new(&file_id, 0);
                    Ok((Box::new(reader), 0))
                } else {
                    let msg = format!("file has not body! file={}, size={}", file_id, total_size);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
                }
            }
        }
    }

    pub async fn put_chunk(
        &self,
        chunk_id: &ChunkId,
        chunk: &dyn Chunk,
        referer_object: Vec<NDNDataRefererObject>,
    ) -> BuckyResult<()> {
        assert!(chunk_id.len() == chunk.get_len());

        self.chunk_manager.put_chunk(chunk_id, chunk).await?;

        self.ndc
            .insert_chunk(&InsertChunkRequest {
                chunk_id: chunk_id.clone(),
                state: ChunkState::Ready,
                ref_objects: None,
                trans_sessions: None,
                flags: 0,
            })
            .await?;

        let request = AddTrackerPositonRequest {
            id: chunk_id.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::ChunkManager,
            flags: 0,
        };
        if let Err(e) = self.tracker.add_position(&request).await {
            if e.code() != BuckyErrorCode::AlreadyExists {
                error!("add to tracker failed for {}", e);
                return Err(e);
            }
        };

        // 尝试更新chunk所在的索引
        let mut req = UpdateChunkRefsRequest {
            chunk_id: chunk_id.to_owned(),
            add_list: vec![],
            remove_list: vec![],
        };

        for referer in referer_object.into_iter() {
            match referer.object_id.obj_type_code() {
                ObjectTypeCode::File => {
                    let r = ChunkObjectRef {
                        object_id: referer.object_id,
                        relation: ChunkObjectRelation::FileBody,
                    };

                    req.add_list.push(r);
                }
                ObjectTypeCode::Dir => {
                    if referer.inner_path.is_none() {
                        let r = ChunkObjectRef {
                            object_id: referer.object_id,
                            relation: ChunkObjectRelation::DirMeta,
                        };

                        req.add_list.push(r);
                    } else {
                        // TODO
                        // dir存在内部路径，所以指向了内部的文件，这里暂时不处理
                    }
                }
                code @ _ => {
                    warn!(
                        "unsupport chunk ref objects type! id={}, type={:?}",
                        referer.object_id, code
                    );
                }
            }
        }

        if !req.add_list.is_empty() {
            self.ndc.update_chunk_ref_objects(&req).await?;
        }

        info!(
            "local put chunk success: chunk={}, len={}",
            chunk_id,
            chunk_id.len()
        );

        Ok(())
    }

    pub async fn get_chunk(
        &self,
        chunk_id: &ChunkId,
        ranges: Option<Vec<Range<u64>>>,
    ) -> BuckyResult<Option<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)>> {
        let ret = self.reader.get(chunk_id).await;

        if let Err(e) = ret {
            if e.code() == BuckyErrorCode::NotFound {
                info!("local get chunk but not found: chunk={}, {}", chunk_id, e);
                return Ok(None);
            }
            error!("local get chunk error! chunk={}, {}", chunk_id, e);
            return Err(e);
        }

        let data = ret.unwrap();
        debug!(
            "local get chunk success! chunk={}, len={}, ranges={:?}",
            chunk_id,
            chunk_id.len(),
            ranges,
        );

        let ret = if let Some(ranges) = ranges {
            let length = RangeHelper::sum(&ranges);
            let range_reader = ChunkReadWithRanges::new(data, ranges);
            (
                Box::new(range_reader) as Box<dyn Read + Unpin + Send + Sync + 'static>,
                length,
            )
        } else {
            (
                AsyncReadWithSeekAdapter::new(data).into_reader(),
                chunk_id.len() as u64,
            )
        };

        Ok(Some(ret))
    }

    pub async fn get_chunk_meta(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<Option<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)>> {
        let ret = self
            .chunk_manager
            .get_chunk_meta(chunk_id, ChunkType::MMapChunk)
            .await;

        if let Err(e) = ret {
            if e.code() == BuckyErrorCode::NotFound {
                info!("local get chunk but not found: chunk={}, {}", chunk_id, e);
                return Ok(None);
            }
            error!("local get chunk error! chunk={}, {}", chunk_id, e);
            return Err(e);
        }

        let data = ret.unwrap();
        debug!(
            "local get chunk success! chunk={}, len={}",
            chunk_id,
            chunk_id.len(),
        );

        let buf = data.to_vec()?;
        let len = buf.len() as u64;
        Ok(Some((Box::new(Cursor::new(buf)), len)))
    }

    pub async fn exist_chunk(&self, chunk_id: &ChunkId) -> bool {
        let exist = self.chunk_manager.exist(chunk_id).await;

        exist
    }

    // 获取chunk列表
    async fn get_chunks(
        &self,
        file_id: &ObjectId,
        total_size: usize,
        chunks: &Vec<ChunkId>,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
        info!(
            "will get chunk list: count={}, total_size={}",
            chunks.len(),
            total_size
        );
        let result = FileChunkListStreamWriter::new(file_id, total_size);

        for (i, chunk_id) in chunks.iter().enumerate() {
            debug!(
                "will local get chunk, index={}, chunk={}, len={}",
                i,
                chunk_id,
                chunk_id.len()
            );
            if chunk_id.len() == 0 {
                // 对于长度为0的chunk，直接认为已经成功
                continue;
            }

            let reader = self.reader.get(chunk_id).await.map_err(|e| {
                if e.code() == BuckyErrorCode::NotFound {
                    warn!(
                        "local get chunk but not found! chunk={}, len={}",
                        chunk_id,
                        chunk_id.len()
                    );
                } else {
                    warn!(
                        "local get chunk error! chunk={}, len={}, {}",
                        chunk_id,
                        chunk_id.len(),
                        e,
                    );
                }

                e
            })?;

            let reader = AsyncReadWithSeekAdapter::new(reader).into_reader();
            result.append(chunk_id, reader);
        }

        Ok((Box::new(result), total_size as u64))
    }

    fn calc_chunks_with_ranges(
        chunks: &Vec<ChunkId>,
        ranges: &Vec<Range<u64>>,
    ) -> (u64, Vec<(ChunkId, Vec<Range<u64>>)>) {
        let mut start = 0;
        let mut result = vec![];
        let mut length = 0;
        for chunk_id in chunks {
            let chunk_range = Range {
                start,
                end: start + chunk_id.len() as u64,
            };
            start = chunk_range.end;

            let list = RangeHelper::intersect_list(&chunk_range, ranges);
            if list.is_empty() {
                continue;
            }

            length += RangeHelper::sum(&list);
            result.push((chunk_id.to_owned(), list));
        }

        (length, result)
    }

    async fn get_chunks_with_range(
        &self,
        file_id: &ObjectId,
        total_size: usize,
        chunks: &Vec<ChunkId>,
        ranges: Vec<Range<u64>>,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
        info!(
            "will get chunk list: count={}, total_size={}, range={:?}",
            chunks.len(),
            total_size,
            ranges,
        );

        let (length, list) = Self::calc_chunks_with_ranges(&chunks, &ranges);

        // length maybe > total_size! if there are some overlap ranges
        info!("calc all range len: {}", length);

        let result = FileChunkListStreamWriter::new(file_id, length as usize);

        for (i, (chunk_id, ranges)) in list.into_iter().enumerate() {
            debug!(
                "will local get chunk with range, index={}, chunk={}, len={}, ranges={:?}",
                i,
                chunk_id,
                chunk_id.len(),
                ranges,
            );
            if chunk_id.len() == 0 {
                // 对于长度为0的chunk，直接认为已经成功
                continue;
            }
            assert!(!ranges.is_empty());

            let reader = self.reader.get(&chunk_id).await.map_err(|e| {
                if e.code() == BuckyErrorCode::NotFound {
                    warn!(
                        "local get chunk but not found! chunk={}, len={}",
                        chunk_id,
                        chunk_id.len()
                    );
                } else {
                    warn!(
                        "local get chunk error! chunk={}, len={}, {}",
                        chunk_id,
                        chunk_id.len(),
                        e,
                    );
                }

                e
            })?;

            let range_reader = ChunkReadWithRanges::new(reader, ranges);
            result.append(&chunk_id, Box::new(range_reader));
        }

        Ok((Box::new(result), length))
    }

    pub async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        let list = match req.param {
            NDNQueryFileParam::File(id) => {
                let file_id = FileId::try_from(id).map_err(|e| {
                    let msg = format!(
                        "query_file's object_id need file_id: id={}, type={:?}, {}",
                        id,
                        id.obj_type_code(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                })?;

                let req = GetFileByFileIdRequest {
                    file_id,
                    flags: req.common.flags,
                };

                match self.ndc.get_file_by_file_id(&req).await? {
                    Some(item) => vec![item],
                    None => vec![],
                }
            }
            NDNQueryFileParam::Hash(hash) => {
                let req = GetFileByHashRequest {
                    hash: hash.to_string(),
                    flags: req.common.flags,
                };

                match self.ndc.get_file_by_hash(&req).await? {
                    Some(item) => vec![item],
                    None => vec![],
                }
            }
            NDNQueryFileParam::QuickHash(quick_hash) => {
                let req = GetFileByQuickHashRequest {
                    quick_hash,
                    length: 0,
                    flags: req.common.flags,
                };

                self.ndc.get_files_by_quick_hash(&req).await?
            }
            NDNQueryFileParam::Chunk(chunk_id) => {
                let req = GetFileByChunkRequest {
                    chunk_id,
                    flags: req.common.flags,
                };

                self.ndc.get_files_by_chunk(&req).await?
            }
        };

        let list = list
            .into_iter()
            .map(|item| NDNQueryFileInfo {
                file_id: item.file_id,
                hash: item.hash,
                length: item.length,
                flags: item.flags,
                owner: item.owner,
                quick_hash: item.quick_hash,
                ref_dirs: item.dirs,
            })
            .collect();

        let resp = NDNQueryFileInputResponse { list };

        Ok(resp)
    }
}
