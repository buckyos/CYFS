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
    pub(crate) fn new(
        validator: GlobalStateValidatorManager,
        next: NONInputProcessorRef,
    ) -> NONInputProcessorRef {
        let ret = Self { validator, next };
        Arc::new(Box::new(ret))
    }

    async fn validate(
        &self,
        source: &RequestSourceInfo,
        req_path: &str,
        object_id: &ObjectId,
    ) -> BuckyResult<()> {
        // debug!("will validate object: req_path={}, object={}", req_path, object_id);
        
        let req_path = RequestGlobalStatePath::from_str(req_path)?;

        // 同zone+同dec，或者同zone+system，那么不需要validate
        if source.is_current_zone() {
            if source.check_target_dec_permission(&req_path.dec_id) {
                return Ok(());
            }
        }

        let category = req_path.category();
        let dec_id = req_path.dec(source).to_owned();

        let root = match req_path.global_state_root {
            Some(root) => match root {
                RequestGlobalStateRoot::GlobalRoot(id) => GlobalStateValidateRoot::GlobalRoot(id),
                RequestGlobalStateRoot::DecRoot(id) => GlobalStateValidateRoot::DecRoot(id),
            },
            None => GlobalStateValidateRoot::None,
        };

        let inner_path = req_path.req_path.unwrap_or("/".to_owned());

        let validate_req = GlobalStateValidateRequest {
            dec_id,
            root,
            object_id: Some(object_id.clone()),
            inner_path,
        };

        self.validator
            .get_validator(category)
            .validate(validate_req)
            .await?;

        Ok(())
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
            self.validate(&req.common.source, req_path, &req.object_id).await?;
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
            self.validate(&req.common.source, req_path, &req.object_id).await?;
        }

        self.next.delete_object(req).await
    }
}
