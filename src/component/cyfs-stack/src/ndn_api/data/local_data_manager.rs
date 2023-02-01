use super::TargetDataManager;
use cyfs_bdt_ext::*;
use cyfs_base::*;
use cyfs_chunk_cache::ChunkType;
use cyfs_chunk_lib::Chunk;
use cyfs_lib::*;

use async_std::io::{Cursor, Read};
use once_cell::sync::OnceCell;
use std::convert::TryFrom;
use std::ops::Range;

pub(crate) struct LocalDataManager {
    named_data_components: NamedDataComponents,

    target_data_manager: OnceCell<TargetDataManager>,
}

impl LocalDataManager {
    pub(crate) fn new(named_data_components: &NamedDataComponents) -> Self {
        Self {
            named_data_components: named_data_components.to_owned(),
            target_data_manager: OnceCell::new(),
        }
    }

    fn target_data_manager(&self) -> &TargetDataManager {
        self.target_data_manager.get_or_init(|| {
            let target = self
                .named_data_components
                .bdt_stack()
                .local_device_id()
                .to_owned();
            let target_desc = self
                .named_data_components
                .bdt_stack()
                .local_const()
                .to_owned();

            let context =
                ContextManager::create_download_context_from_target_sync("", target, target_desc);

            TargetDataManager::new(
                self.named_data_components.bdt_stack().clone(),
                self.named_data_components.chunk_manager.clone(),
                context,
            )
        })
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
        self.target_data_manager()
            .get_file(source, file_obj, group, ranges)
            .await
    }

    pub async fn put_chunk(
        &self,
        chunk_id: &ChunkId,
        chunk: Box<dyn Chunk>,
        referer_object: Vec<NDNDataRefererObject>,
    ) -> BuckyResult<()> {
        assert!(chunk_id.len() == chunk.get_len());

        self.named_data_components
            .chunk_manager
            .put_chunk(chunk_id, chunk)
            .await?;

        self.named_data_components
            .ndc
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
        if let Err(e) = self
            .named_data_components
            .tracker
            .add_position(&request)
            .await
        {
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
            self.named_data_components
                .ndc
                .update_chunk_ref_objects(&req)
                .await?;
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
        source: &RequestSourceInfo,
        chunk_id: &ChunkId,
        group: Option<&str>,
        ranges: Option<Vec<Range<u64>>>,
    ) -> BuckyResult<(
        Box<dyn Read + Unpin + Send + Sync + 'static>,
        u64,
        Option<String>,
    )> {
        self.target_data_manager()
            .get_chunk(source, chunk_id, group, ranges)
            .await
    }

    pub async fn get_chunk_meta(
        &self,
        chunk_id: &ChunkId,
    ) -> BuckyResult<(Box<dyn Read + Unpin + Send + Sync + 'static>, u64)> {
        let data = self
            .named_data_components
            .chunk_manager
            .get_chunk_meta(chunk_id, ChunkType::MMapChunk)
            .await?;

        debug!(
            "local get chunk success! chunk={}, len={}",
            chunk_id,
            chunk_id.len(),
        );

        let buf = data.to_vec()?;
        let len = buf.len() as u64;
        Ok((Box::new(Cursor::new(buf)), len))
    }

    pub async fn exist_chunk(&self, chunk_id: &ChunkId) -> bool {
        let exist = self
            .named_data_components
            .chunk_manager
            .exist(chunk_id)
            .await;

        exist
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

                match self
                    .named_data_components
                    .ndc
                    .get_file_by_file_id(&req)
                    .await?
                {
                    Some(item) => vec![item],
                    None => vec![],
                }
            }
            NDNQueryFileParam::Hash(hash) => {
                let req = GetFileByHashRequest {
                    hash: hash.to_string(),
                    flags: req.common.flags,
                };

                match self
                    .named_data_components
                    .ndc
                    .get_file_by_hash(&req)
                    .await?
                {
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

                self.named_data_components
                    .ndc
                    .get_files_by_quick_hash(&req)
                    .await?
            }
            NDNQueryFileParam::Chunk(chunk_id) => {
                let req = GetFileByChunkRequest {
                    chunk_id,
                    flags: req.common.flags,
                };

                self.named_data_components
                    .ndc
                    .get_files_by_chunk(&req)
                    .await?
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
