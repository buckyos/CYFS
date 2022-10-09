use super::super::acl::{NDNAclLocalInputProcessor, NDNInputAclSwitcher};
use super::super::data::{zero_bytes_reader, LocalDataManager};
use super::object_loader::NDNObjectLoader;
use crate::ndn::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::cache::NamedDataCache;
use futures::AsyncReadExt;

use cyfs_chunk_cache::ChunkManager;
use cyfs_chunk_lib::{ChunkMeta, MemRefChunk};
use std::convert::TryFrom;
use std::sync::Arc;

pub(crate) struct NDCLevelInputProcessor {
    data_manager: LocalDataManager,

    object_loader: NDNObjectLoader,
}

impl NDCLevelInputProcessor {
    pub fn new_raw(
        chunk_manager: Arc<ChunkManager>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,

        // 不带权限的non处理器
        non_processor: NONInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let object_loader = NDNObjectLoader::new(non_processor);

        let ret = Self {
            data_manager: LocalDataManager::new(chunk_manager, ndc, tracker),
            object_loader,
        };

        Arc::new(Box::new(ret))
    }

    // 创建一个带本地权限的processor
    pub fn new_local(
        chunk_manager: Arc<ChunkManager>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        raw_noc_processor: NONInputProcessorRef,
    ) -> NDNInputProcessorRef {
        // 不带input acl的处理器
        let raw_processor = Self::new_raw(chunk_manager, ndc, tracker, raw_noc_processor);

        // 带local input acl的处理器
        let acl_processor = NDNAclLocalInputProcessor::new(raw_processor.clone());

        // 使用acl switcher连接
        let processor = NDNInputAclSwitcher::new(acl_processor, raw_processor);

        processor
    }

    async fn put_chunk(
        &self,
        mut req: NDNPutDataInputRequest,
    ) -> BuckyResult<NDNPutDataInputResponse> {
        let chunk_id = ChunkId::try_from(&req.object_id).unwrap();

        // 首先检查本地是否已经存在了
        if self.data_manager.exist_chunk(&chunk_id).await {
            info!("chunk already exists! chunk={}", chunk_id);
            return Ok(NDNPutDataInputResponse {
                result: NDNPutDataResult::AlreadyExists,
            });
        }

        let mut chunk_raw = vec![];
        let len = req.data.read_to_end(&mut chunk_raw).await.map_err(|e| {
            let msg = format!(
                "read chunk buffer from request error! chunk={}, {}",
                chunk_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        match req.data_type {
            NDNDataType::Mem => {
                // TODO 校验hash
                if len != chunk_id.len() {
                    let msg = format!(
                        "unmatch chunk buffer length! read={}, chunk len={}",
                        len,
                        chunk_id.len(),
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }

                self.data_manager
                    .put_chunk(
                        &chunk_id,
                        &MemRefChunk::from(chunk_raw.as_slice()),
                        req.common.referer_object,
                    )
                    .await?;
            }
            NDNDataType::SharedMem => {
                let chunk = ChunkMeta::clone_from_slice(chunk_raw.as_slice())?
                    .to_chunk()
                    .await?;
                self.data_manager
                    .put_chunk(&chunk_id, chunk.as_ref(), req.common.referer_object)
                    .await?;
            }
        }
        Ok(NDNPutDataInputResponse {
            result: NDNPutDataResult::Accept,
        })
    }

    // put_data目前只支持chunk
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        match req.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                // chunk
                self.put_chunk(req).await
            }
            code @ _ => {
                let msg = format!(
                    "ndn put_chunk only support chunk type! id={}, type={:?}",
                    req.object_id, code,
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    // 从本地noc查找file对象
    async fn get_file(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let (file_id, file) = self.object_loader.get_file_object(&req, None).await?;
        assert_eq!(file_id, file.desc().calculate_id());

        let total_size = file.desc().content().len();

        // process range
        let mut need_process = true;
        let mut ranges = None;
        let mut resp_range = None;
        if let Some(ref range) = req.range {
            resp_range = range.convert_to_response(total_size);
            match &resp_range {
                Some(range) => match range {
                    NDNDataResponseRange::Range(r) => {
                        ranges = Some(r.0.clone());
                    }
                    _ => {
                        need_process = false;
                    }
                },
                None => {
                    // parse range param but empty, will get the whole file
                }
            }
        } else {
            // no range param specified, will get the whole file
        }

        let (data, length) = if need_process {
            self.data_manager.get_file(&file, ranges).await?
        } else {
            (zero_bytes_reader(), 0)
        };

        let resp = NDNGetDataInputResponse {
            object_id: file_id,
            owner_id: file.desc().owner().to_owned(),
            attr: None,
            length,
            range: resp_range,
            data,
        };

        Ok(resp)
    }

    async fn get_chunk(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let chunk_id = ChunkId::try_from(&req.object_id).unwrap();
        let total_size = chunk_id.len() as u64;

        // process range
        let mut need_process = true;
        let mut ranges = None;
        let mut resp_range = None;
        if let Some(ref range) = req.range {
            resp_range = range.convert_to_response(total_size);
            match &resp_range {
                Some(range) => match range {
                    NDNDataResponseRange::Range(r) => {
                        ranges = Some(r.0.clone());
                    }
                    _ => {
                        need_process = false;
                    }
                },
                None => {
                    // parse range param but empty, will get the whole chunk
                }
            }
        } else {
            // no range param specified, will get the whole chunk
        }

        let ret = if need_process {
            match req.data_type {
                NDNDataType::Mem => self.data_manager.get_chunk(&chunk_id, ranges).await?,
                NDNDataType::SharedMem => self.data_manager.get_chunk_meta(&chunk_id).await?,
            }
        } else {
            Some((zero_bytes_reader(), 0))
        };

        if let Some((data, length)) = ret {
            let resp = NDNGetDataInputResponse {
                object_id: req.object_id,
                owner_id: None,
                attr: None,
                range: resp_range,
                length,
                data,
            };
            Ok(resp)
        } else {
            let msg = format!(
                "ndn get_chunk from local but not found! id={}",
                req.object_id
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        }
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {

        match req.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                // 加载chunk
                self.get_chunk(req).await
            }
            ObjectTypeCode::File => self.get_file(req).await,
            ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                // 如果是dir，那么必须指定目标文件的inner_path
                if req.inner_path.is_none() {
                    let msg = format!(
                        "ndc get_chunk from {:?} but inner_path is empty! id={}",
                        req.object_id.obj_type_code(),
                        req.object_id,
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                }

                self.get_file(req).await
            }
            code @ _ => {
                let msg = format!(
                    "ndn get_chunk only support chunk/file/dir object type! id={}, type={:?}",
                    req.object_id, code,
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        let msg = format!("ndc delete_data not support yet! id={}", req.object_id,);
        error!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        self.data_manager.query_file(req).await
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDCLevelInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDCLevelInputProcessor::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDCLevelInputProcessor::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        NDCLevelInputProcessor::delete_data(&self, req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        NDCLevelInputProcessor::query_file(&self, req).await
    }
}
