use super::verifier::NDNRefererVerifier;
use crate::acl::*;
use crate::ndn::*;
use crate::ndn_api::ndc::NDNObjectLoader;
use cyfs_bdt_ext::ChunkStoreReader;
use crate::ndn_api::NDNForwardObjectData;
use crate::non::NONInputProcessorRef;
use crate::non_api::NONGlobalStateValidator;

use cyfs_base::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::str::FromStr;

pub(crate) struct NDNAclInputProcessor {
    acl: AclManagerRef,
    loader: OnceCell<NDNObjectLoader>,
    next: NDNInputProcessorRef,

    // only used for validate on req_path+chunk mode
    validator: NONGlobalStateValidator,
    verifier: NDNRefererVerifier,
}

impl NDNAclInputProcessor {
    pub fn new(
        acl: AclManagerRef,
        chunk_reader: ChunkStoreReader,
        next: NDNInputProcessorRef,
    ) -> Self {
        let verifier = NDNRefererVerifier::new(chunk_reader);
        Self {
            validator: NONGlobalStateValidator::new(acl.global_state_validator().to_owned()),
            verifier,
            loader: OnceCell::new(),
            next,
            acl,
        }
    }

    pub fn bind_non_processor(&self, non_processor: NONInputProcessorRef) {
        let loader = NDNObjectLoader::new(non_processor);
        if let Err(_) = self.loader.set(loader) {
            unreachable!();
        }
    }

    fn loader(&self) -> BuckyResult<&NDNObjectLoader> {
        match self.loader.get() {
            Some(loader) => Ok(loader),
            None => {
                let msg = format!("ndn acl not initialized yet!");
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        }
    }

    async fn check_access(
        &self,
        req_path: &RequestGlobalStatePath,
        source: &RequestSourceInfo,
        op_type: RequestOpType,
    ) -> BuckyResult<ObjectId> {
        debug!(
            "will check access: req_path={}, source={}, {:?}",
            req_path, source, op_type
        );

        // 同zone+同dec，或者同zone+system，那么不需要校验权限
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

    async fn on_get_chunk(
        &self,
        req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputRequest> {
        debug!("will check get_chunk access: req={}", req,);

        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::Chunk);

        if req.common.referer_object.is_empty() {
            let req_path = match &req.common.req_path {
                Some(req_path) => Some(RequestGlobalStatePath::from_str(req_path)?),
                None => None,
            };
            
            if req_path.is_none() {
                // 同zone内，不指定referer_object，也不指定req_path，可直接使用chunk_id访问
                if req.common.source.is_current_zone() {
                    return Ok(req);
                }

                let msg = format!(
                    "get_data with chunk_id but referer_object and req_path is empty! chunk={}",
                    req.object_id
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
            }

            // 直接使用req_path + chunk_id进行校验，也即要求chunk_id挂到root_state上
            self.check_access(
                req_path.as_ref().unwrap(),
                &req.common.source,
                RequestOpType::Read,
            )
            .await?;

            // validate the req_path + chunk
            self.validator
                .validate(&req.common.source, req_path.unwrap(), &req.object_id)
                .await?;
        } else {
            // 直接通过本地non加载引用的目标object，在non里面会check_access of object & verify object is on root-state
            let object = self.loader()?.get_file_or_dir_object(&req, None).await?;

            // 需要校验chunk_id和引用对象是否存在关联
            self.verifier
                .verify_referer(
                    &object.object_id,
                    object.object(),
                    req.object_id.as_chunk_id(),
                )
                .await?;
        }

        Ok(req)
    }

    async fn on_get_file(
        &self,
        mut req: NDNGetDataInputRequest,
    ) -> BuckyResult<NDNGetDataInputRequest> {
        debug!("will check get_file access: req={}", req);

        // During some request from front /o and /a, the object will been fetched at first, then load the ndn files
        // at this case we should not check the access anymore
        if let Some(user_data) = &req.common.user_data {
            let udata = NDNForwardObjectData::from_any(user_data);
            assert_eq!(udata.file_id, req.object_id);

            return Ok(req);
        }

        // assert!(req.common.user_data.is_none());

        let (file_id, file) = self.loader()?.get_file_object(&req, None).await?;
        assert_eq!(file_id, file.desc().calculate_id());
        let user_data = NDNForwardObjectData { file, file_id };
        req.common.user_data = Some(user_data.to_any());

        Ok(req)
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
        let req = match req.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => self.on_get_chunk(req).await?,
            ObjectTypeCode::File | ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                self.on_get_file(req).await?
            }
            code @ _ => {
                let msg = format!(
                    "ndn get data but unsupport object type: id={}, type={:?}",
                    req.object_id, code,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

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
