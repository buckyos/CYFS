use crate::non::*;
use crate::root_state_api::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;
use std::sync::Arc;

pub struct NONGlobalStateValidator {
    validator: GlobalStateValidatorManager,
    next: NONInputProcessorRef,
}

impl NONGlobalStateValidator {
    pub(crate) fn new(validator: GlobalStateValidatorManager, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { validator, next };
        Arc::new(Box::new(ret))
    }

    async fn validate(
        &self,
        req_path: &str,
        object_id: &ObjectId,
    ) -> BuckyResult<GlobalStateValidateResponse> {
        let global_state_common = RequestGlobalStateCommon::from_str(req_path)?;
        let category = global_state_common.category();
        let dec_id = global_state_common.dec().to_owned();

        let root = match global_state_common.global_state_root {
            Some(root) => match root {
                RequestGlobalStateRoot::GlobalRoot(id) => GlobalStateValidateRoot::GlobalRoot(id),
                RequestGlobalStateRoot::DecRoot(id) => GlobalStateValidateRoot::DecRoot(id),
            },
            None => GlobalStateValidateRoot::None,
        };

        let inner_path = global_state_common.req_path.unwrap_or("/".to_owned());

        let validate_req = GlobalStateValidateRequest {
            dec_id,
            root,
            object_id: Some(object_id.clone()),
            inner_path,
        };

        self.validator.get_validator(category).validate(validate_req).await
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
        if !req.common.source.is_current_zone() {
            if let Some(req_path) = &req.common.req_path {
                let _resp = self.validate(req_path, &req.object_id).await?;
            }
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
        if !req.common.source.is_current_zone() {
            if let Some(req_path) = &req.common.req_path {
                let _resp = self.validate(req_path, &req.object_id).await?;
            }
        }

        self.next.delete_object(req).await
    }
}
