use super::verifier::NDNChunkVerifier;
use crate::acl::*;
use crate::ndn::*;
use crate::ndn_api::ndc::NDNObjectLoader;
use crate::ndn_api::LocalDataManager;
use crate::non::NONInputProcessorRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;
use std::sync::Arc;
use once_cell::sync::OnceCell;

pub(crate) struct NDNAclInputProcessor {
    acl: AclManagerRef,
    loader: OnceCell<NDNObjectLoader>,
    next: NDNInputProcessorRef,

    verifier: NDNChunkVerifier,
}

impl NDNAclInputProcessor {
    pub fn new(
        acl: AclManagerRef,
        data_manager: LocalDataManager,
        next: NDNInputProcessorRef,
    ) -> NDNInputProcessorRef {
        let verifier = NDNChunkVerifier::new(data_manager);
        let ret = Self {
            acl,
            verifier,
            loader: OnceCell::new(),
            next,
        };
        Arc::new(Box::new(ret))
    }

    pub fn bind_non_processor(&self, non_processor: NONInputProcessorRef) {
        let loader = NDNObjectLoader::new(non_processor);
        if let Some(_) = self.loader.set(loader) {
            unreachable!();
        }
    }

    fn loader(&self) -> BuckyResult<&NDNObjectLoader> {
        match self.loader.get() {
            Some(loader) => Ok(loader),
            None => {
                let msg = format!(
                    "ndn acl not initialized yet!"
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        }
    }

    async fn check_access(
        &self,
        req_path: &str,
        source: &RequestSourceInfo,
        op_type: RequestOpType,
    ) -> BuckyResult<ObjectId> {
        debug!(
            "will check access: req_path={}, source={}, {:?}",
            req_path, source, op_type
        );

        let req_path = RequestGlobalStatePath::from_str(req_path)?;

        // 同zone+同dec，或者同zone+system，那么不需要校验rmeta权限
        if source.is_current_zone() {
            if source.check_target_dec_permission(&req_path.dec_id) {
                return Ok(req_path.dec(source).to_owned());
            }
        }

        self.acl
            .global_state_meta()
            .check_access(source, &req_path, op_type)
            .await?;

        Ok(req_path.dec(source).to_owned())
    }

    async fn on_get_chunk(&self, req: &NDNGetDataInputRequest) -> BuckyResult<()> {
        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::Chunk);

        if req.common.referer_object.is_empty() {
            // 直接使用req_path + chunk_id进行校验，也即要求chunk_id挂到root_state上
            self.check_access(
                req.common.req_path.as_ref().unwrap(),
                &req.common.source,
                RequestOpType::Read,
            )
            .await?;
        } else {
            let object = self.loader()?.get_file_or_dir_object(&req, None).await?;

            self.verifier
                .verify_chunk(
                    &object.object_id,
                    object.object(),
                    req.object_id.as_chunk_id(),
                )
                .await?;
        }

        Ok(())
    }

    async fn on_get_file(&self, req: &NDNGetDataInputRequest) -> BuckyResult<()> {
        let _object = self.loader()?.get_file_object(&req, None).await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl NDNInputProcessor for NDNAclInputProcessor {
    async fn put_data(&self, req: NDNPutDataInputRequest) -> BuckyResult<NDNPutDataInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "put_data only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.put_data(req).await
    }

    async fn get_data(&self, req: NDNGetDataInputRequest) -> BuckyResult<NDNGetDataInputResponse> {
        // FIXME 设计合理的权限，需要配合object_id和referer_objects
        // warn!(">>>>>>>>>>>>>>>>>>>>>>get_data acl not impl!!!!");

        match req.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                self.on_get_chunk(&req).await?;
            }
            ObjectTypeCode::File | ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                self.on_get_file(&req).await?;
            }
            code @ _ => {
                let msg = format!(
                    "ndn get data but unsupport object type: id={}, type={:?}",
                    req.object_id, code,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        }

        self.next.get_data(req).await
    }

    async fn delete_data(
        &self,
        req: NDNDeleteDataInputRequest,
    ) -> BuckyResult<NDNDeleteDataInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!(
                "delete_data only allow within the same zone! {}",
                req.object_id
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.delete_data(req).await
    }

    async fn query_file(
        &self,
        req: NDNQueryFileInputRequest,
    ) -> BuckyResult<NDNQueryFileInputResponse> {
        if !req.common.source.is_current_zone() {
            let msg = format!("query_file only allow within the same zone! {}", req);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        self.next.query_file(req).await
    }
}
