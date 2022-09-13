use crate::non::*;
use crate::root_state_api::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;


pub struct NONGlobalStateValidator {
    validator: GlobalStateValidator,
    next: NONInputProcessorRef,
}

impl NONGlobalStateValidator {
    pub(crate) fn new(validator: GlobalStateValidator, next: NONInputProcessorRef) -> Self {
        Self { validator, next }
    }

    async fn validate(
        &self,
        req_path: &str,
        object_id: &ObjectId,
    ) -> BuckyResult<GlobalStateValidateResponse> {
        let global_state_common = RequestGlobalStateCommon::from_str(req_path)?;

        let root = match global_state_common.global_state_root {
            Some(root) => match root {
                RequestGlobalStateRoot::GlobalRoot(id) => GlobalStateValidateRoot::GlobalRoot(id),
                RequestGlobalStateRoot::DecRoot(id) => GlobalStateValidateRoot::DecRoot(id),
            },
            None => GlobalStateValidateRoot::None,
        };

        let inner_path = global_state_common.req_path.unwrap_or("/".to_owned());

        let validate_req = GlobalStateValidateRequest {
            dec_id: global_state_common.dec_id,
            root,
            object_id: Some(object_id.clone()),
            inner_path,
        };

        self.validator.validate(validate_req).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONGlobalStateValidator {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let _resp = self.validate(req_path, &req.object_id).await?;
        }

        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            let _resp = self.validate(req_path, &req.object_id).await?;
        }

        self.next.delete_object(req).await
    }
}
