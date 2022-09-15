use crate::acl::AclManagerRef;
use crate::non::*;
use crate::rmeta_api::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;

pub(crate) struct NONAclInputProcessor {
    acl: AclManagerRef,
    next: NONInputProcessorRef,
}

impl NONAclInputProcessor {
    pub fn new(acl: AclManagerRef, next: NONInputProcessorRef) -> NONInputProcessorRef {
        let ret = Self { acl, next };
        Arc::new(Box::new(ret))
    }

    async fn check_call_access(
        &self,
        req_path: &str,
        source: &RequestSourceInfo,
    ) -> BuckyResult<()> {
        let global_state_common = RequestGlobalStatePath::from_str(req_path)?;

        // 同zone+同dec，或者同zone+system，那么不需要校验rmeta权限
        if source.is_current_zone() {
            if source.check_target_dec_permission(&global_state_common.dec_id) {
                return Ok(());
            }
        }

        let rmeta = self
            .acl
            .global_state_meta()
            .get_meta_manager(global_state_common.category());
        let dec_rmeta = rmeta.get_global_state_meta(&global_state_common.dec_id, false).await.map_err(|e| {
            let msg = format!("non check rmeta but target dec rmeta not found or with error! {}, target_dec={:?}, {}", req_path, global_state_common.dec_id, e);
            warn!("{}", msg);
            BuckyError::new(BuckyErrorCode::PermissionDenied, msg)
        })?;

        let check_req = GlobalStateAccessRequest {
            dec: Cow::Borrowed(&global_state_common.dec()),
            path: global_state_common.req_path(),
            source: Cow::Borrowed(source),
            op_type: RequestOpType::Call,
        };

        if let Err(e) = dec_rmeta.check_access(check_req) {
            error!(
                "get_object check rmeta but been rejected! {}, {}",
                req_path, e
            );
            return Err(e);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONAclInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "put_object only allow within the same zone! {}",
                req.object.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.put_object(req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        self.next.get_object(req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if let Some(req_path) = &req.common.req_path {
            self.check_call_access(req_path, &req.common.source).await?;
        }

        self.next.post_object(req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!("select_object only allow within the same zone! {}", req);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.select_object(req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "delete_object only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.delete_object(req).await
    }
}
