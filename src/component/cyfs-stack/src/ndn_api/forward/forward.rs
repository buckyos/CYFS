use super::super::data::*;
use crate::ndn::*;
use cyfs_base::*;
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_lib::*;
use cyfs_bdt::StackGuard;

use std::convert::TryFrom;
use std::sync::Arc;

pub(crate) struct NDNForwardObjectData {
    pub file: File,
    pub file_id: ObjectId,
}

pub(crate) type NDNForwardObjectDataRef = Arc<NDNForwardObjectData>;

impl NDNForwardObjectData {
    pub fn to_any(self) -> NDNInputRequestUserData {
        Arc::new(self)
    }
    pub fn from_any(ud: &NDNInputRequestUserData) -> NDNForwardObjectDataRef {
        ud.clone().downcast::<Self>().unwrap()
    }
}

pub(crate) struct NDNForwardDataOutputProcessor {
    data_manager: TargetDataManager,
}

impl NDNForwardDataOutputProcessor {
    pub fn new(
        bdt_stack: StackGuard,
        chunk_manager: ChunkManagerRef,
        target: DeviceId,
    ) -> NDNInputProcessorRef {
        let data_manager = TargetDataManager::new(bdt_stack, chunk_manager, target);
        let ret = Self { data_manager };

        Arc::new(Box::new(ret))
    }

    // put_data目前只支持local
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        let msg = format!(
            "ndn put_data to target not support! chunk={}, target={:?}",
            req.object_id,
            self.data_manager.target(),
        );
        error!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
    }

    async fn get_file(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        // 上一级处理器已经正确设置了user_data
        let udata = NDNForwardObjectData::from_any(req.common.user_data.as_ref().unwrap());

        let file = &udata.file;
        let total_size = file.desc().content().len();

        assert_eq!(udata.file_id, file.desc().calculate_id());

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
            let meta = BdtDataRefererInfo {
                // FIXME: set target field from o link
                target: None, 
                object_id: req.object_id.clone(),
                inner_path: req.inner_path.clone(),
                dec_id: req.common.source.get_opt_dec().cloned(),
                req_path: req.common.req_path,
                referer_object: req.common.referer_object,
                flags: req.common.flags,
            };

            self.data_manager.get_file(&file, ranges, Some(&meta)).await?
        } else {
            (zero_bytes_reader(), 0)
        };

        let resp = NDNGetDataInputResponse {
            object_id: udata.file_id,
            owner_id: file.desc().owner().to_owned(),
            attr: None,
            range: resp_range,
            length,
            data,
        };

        Ok(resp)
    }

    async fn get_chunk(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        let chunk_id = ChunkId::try_from(&req.object_id).unwrap();
        let total_size = chunk_id.len() as u64;

        // process with ranges
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
        } else{
            // no range param specified, will get the whole chunk
        }

        let (data, length) = if need_process {
            let meta = BdtDataRefererInfo {
                // FIXME: set target field from o link
                target: None, 
                object_id: req.object_id.clone(),
                inner_path: None, // 直接获取chunk不存在inner_path
                dec_id: req.common.source.get_opt_dec().cloned(),
                req_path: req.common.req_path,
                referer_object: req.common.referer_object,
                flags: req.common.flags,
            };

            self.data_manager
                .get_chunk(&chunk_id, ranges, Some(&meta))
                .await
                .map_err(|e| {
                    error!(
                        "ndn get_chunk from target failed! chunk={}, target={:?}, {}",
                        chunk_id,
                        self.data_manager.target(),
                        e
                    );
                    e
                })?
        } else {
            (zero_bytes_reader(), 0)
        };

        let resp = NDNGetDataInputResponse {
            object_id: req.object_id,
            owner_id: None,
            attr: None,
            range: resp_range,
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
                        "ndn get_chunk from {:?} but inner_path is empty! id={}, target={:?}",
                        req.object_id.obj_type_code(),
                        req.object_id,
                        self.data_manager.target(),
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                }

                self.get_file(req).await
            }
            code @ _ => {
                let msg = format!(
                    "ndn get_chunk only support chunk/file/dir object type! id={}, target={:?}, type={:?}",
                    req.object_id, self.data_manager.target(), code,
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }
}

// 这里为了性能，直接对接input而不是output
#[async_trait::async_trait]
impl NDNInputProcessor for NDNForwardDataOutputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        NDNForwardDataOutputProcessor::put_data(&self, req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        NDNForwardDataOutputProcessor::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        _req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        unreachable!();
    }

    async fn query_file(
        &self,
        _req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        unreachable!();
    }
}
