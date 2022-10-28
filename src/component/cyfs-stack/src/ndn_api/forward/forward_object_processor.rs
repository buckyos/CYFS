use super::super::ndc::NDNObjectLoader;
use super::forward::*;
use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;


pub(crate) struct NDNForwardObjectProcessor {
    target: DeviceId,
    object_loader: NDNObjectLoader,
    next: NDNInputProcessorRef,
}

impl NDNForwardObjectProcessor {
    pub fn new(
        target: DeviceId,
        // used for load object
        object_loader: Option<NDNObjectLoader>,
        next: NDNInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let ret = Self {
            target,
            object_loader,
            next,
        };

        Arc::new(Box::new(ret))
    }

    async fn get_data(
        &self,
        mut req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {

        // 对file/dir进行加载file_object的操作
        match req.object_id.obj_type_code() {
            ObjectTypeCode::File | ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                if req.common.user_data.is_none() {
                    info!(
                        "will get file for ndn get_data request! object={}, inner_path={:?}, target={:?}",
                        req.object_id, req.inner_path, req.common.target,
                    );
                    let (file_id, file) = self.get_file_object(&req).await?;
    
                    let user_data = NDNForwardObjectData { file, file_id };
                    req.common.user_data = Some(user_data.to_any());
                } else {
                    // already loaded outside
                }
                
            }
            _ => {}
        }

        self.next.get_data(req).await
    }

    async fn get_file_object(&self, req: &NDNGetDataInputRequest) -> BuckyResult<(ObjectId, File)> {
      
        // load target object from non service
        debug!(
            "will get file object from target: object={}, inner_path={:?}, target={:?}",
            req.object_id, req.inner_path, self.target,
        );
        self.object_loader
            .get_file_object(&req, Some(&self.target))
            .await
    }
}

// 这里为了性能，直接对接input而不是output
#[async_trait::async_trait]
impl NDNInputProcessor for NDNForwardObjectProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        Self::get_data(&self, req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        self.next.query_file(req).await
    }
}
