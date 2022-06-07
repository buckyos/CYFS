use crate::ndn::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct NDNRefererVerifier {
    ndc: Arc<Box<dyn NamedDataCache>>,
    next: NDNInputProcessorRef,
}

impl NDNRefererVerifier {
    pub fn new(ndc: Box<dyn NamedDataCache>, next: NDNInputProcessorRef) -> NDNInputProcessorRef {
        let ret = Self {
            ndc: Arc::new(ndc),
            next,
        };

        Arc::new(Box::new(ret))
    }

    async fn verify_chunk(&self, _object_id: &ObjectId, req_common: &NDNInputRequestCommon) {
        if req_common.referer_object.len() > 0 {
            // 明确指定了引用，那么需要依次校验
        } else {
            // 没有指定引用，那么需要向本地ndc查找关联的file和dir
            // 存在以下两种形式:
            // /file_id/chunk_id
            // /dir_id/inner_path/chunk_id
            // 其中/dir_id/inner_path 对应的就是 file_id
            // self.ndc.get_chunk_ref_objects(req)
        }
    }

    async fn verify(&self, object_id: &ObjectId, _req_common: &NDNInputRequestCommon) -> BuckyResult<()> {
        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {

            }
            ObjectTypeCode::File => {
                
            }
            ObjectTypeCode::Dir => {

            }
            _ => {
                
            }
        }

        todo!();
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNRefererVerifier {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        self.next.get_data(req).await
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
