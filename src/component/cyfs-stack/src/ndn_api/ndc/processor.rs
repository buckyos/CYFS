use super::super::acl::NDNAclLocalInputProcessor;
use super::super::data::{zero_bytes_reader, LocalDataManager};
use super::object_loader::NDNObjectLoader;
use crate::acl::AclManagerRef;
use crate::ndn_api::acl::NDNAclInputProcessor;
use crate::ndn_api::NDNForwardObjectData;
use crate::non::*;
use crate::{ndn::*, NamedDataComponents};
use cyfs_base::*;
use cyfs_chunk_cache::MemChunk;
use cyfs_chunk_lib::ChunkMeta;
use cyfs_lib::*;

use futures::AsyncReadExt;
use std::convert::TryFrom;
use std::sync::Arc;

pub(crate) struct NDCLevelInputProcessor {
    data_manager: LocalDataManager,

    object_loader: NDNObjectLoader,
}

impl NDCLevelInputProcessor {
    pub fn new(
        acl: AclManagerRef,
        named_data_components: &NamedDataComponents,

        // router non processor, but only get_object from current stack
        non_processor: NONInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let ret = Self {
            data_manager: LocalDataManager::new(named_data_components),
            object_loader: NDNObjectLoader::new(non_processor.clone()),
        };

        let raw_processor = Arc::new(Box::new(ret) as Box<dyn NDNInputProcessor>);

        // add default ndn acl and chunk verifier
        let acl_processor = NDNAclInputProcessor::new(
            acl,
            named_data_components.new_chunk_store_reader(),
            raw_processor,
        );
        acl_processor.bind_non_processor(non_processor);

        Arc::new(Box::new(acl_processor))
    }

    // 创建一个带本地权限的processor
    pub fn new_local(
        acl: AclManagerRef,
        named_data_components: &NamedDataComponents,
        non_processor: NONInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let processor = Self::new(acl, &named_data_components, non_processor);

        // with current device's acl
        let local_processor = NDNAclLocalInputProcessor::new(processor.clone());

        local_processor
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
                        Box::new(MemChunk::from(chunk_raw)),
                        req.common.referer_object,
                    )
                    .await?;
            }
            NDNDataType::SharedMem => {
                let chunk = ChunkMeta::clone_from_slice(chunk_raw.as_slice())?
                    .to_chunk()
                    .await?;
                self.data_manager
                    .put_chunk(&chunk_id, chunk, req.common.referer_object)
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
        let udata = if let Some(udata) = &req.common.user_data {
            NDNForwardObjectData::from_any(udata)
        } else {
            let (file_id, file) = self.object_loader.get_file_object(&req, None).await?;
            assert_eq!(file_id, file.desc().calculate_id());
            let user_data = NDNForwardObjectData { file, file_id };
            Arc::new(user_data)
        };

        let total_size = udata.file.desc().content().len();

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

        let (data, length, group) = if need_process {
            self.data_manager.get_file(&req.common.source, &udata.file, req.group.as_deref(), ranges).await?
        } else {
            (zero_bytes_reader(), 0, None)
        };

        let resp = NDNGetDataInputResponse {
            object_id: udata.file_id.clone(),
            owner_id: udata.file.desc().owner().to_owned(),
            attr: None,
            length,
            range: resp_range,
            group,
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

        let (data, length, group) = if need_process {
            match req.data_type {
                NDNDataType::Mem => self.data_manager.get_chunk(&req.common.source, &chunk_id, req.group.as_deref(), ranges).await?,
                NDNDataType::SharedMem => {
                    let (reader, len) = self.data_manager.get_chunk_meta(&chunk_id).await?;
                    (reader, len, None)
                }
            }
        } else {
            (zero_bytes_reader(), 0, None)
        };

        let resp = NDNGetDataInputResponse {
            object_id: req.object_id,
            owner_id: None,
            attr: None,
            range: resp_range,
            group,
            length,
            data,
        };
        Ok(resp)
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
