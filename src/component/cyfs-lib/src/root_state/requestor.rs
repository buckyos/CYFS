use super::def::*;
use super::output_request::*;
use super::processor::*;
use crate::base::*;
use crate::non::NONRequestorHelper;
use crate::stack::SharedObjectStackDecID;
use cyfs_base::*;

use http_types::{Method, Request, Response, Url};
use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateRequestor {
    category: GlobalStateCategory,
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl GlobalStateRequestor {
    pub fn new_default_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
    ) -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);
        Self::new_tcp(category, dec_id, &service_addr)
    }

    pub fn new_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        service_addr: &str,
    ) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(category, dec_id, Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new_root_state(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::RootState, dec_id, requestor)
    }

    pub fn new_local_cache(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::LocalCache, dec_id, requestor)
    }

    pub fn new(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/{}/", addr, category.as_str());
        let url = Url::parse(&url).unwrap();

        let ret = Self {
            category,
            dec_id,
            requestor,
            service_url: url,
        };

        ret
    }

    pub fn category(&self) -> &GlobalStateCategory {
        &self.category
    }

    pub fn into_processor(self) -> GlobalStateOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> GlobalStateOutputProcessorRef {
        self.clone().into_processor()
    }

    // TODO: 目前和request的body部分编码一部分冗余的信息
    fn encode_common_headers(
        &self,
        action: RootStateAction,
        com_req: &RootStateOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());

        http_req.insert_header(cyfs_base::CYFS_ROOT_STATE_ACTION, action.to_string());
    }

    // /root_state/root GET
    fn encode_get_current_root_request(
        &self,
        req: &RootStateGetCurrentRootOutputRequest,
    ) -> Request {
        let url = self.service_url.join("root").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(RootStateAction::GetCurrentRoot, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());
        http_req
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootOutputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootOutputResponse> {
        let http_req = self.encode_get_current_root_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: RootStateGetCurrentRootOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!(
                "get current root from root state success: root={}",
                resp.root
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("get current root from root state error! {}", e);
            Err(e)
        }
    }

    // root_state/op_env POST
    fn encode_create_op_env_request(&self, req: &RootStateCreateOpEnvOutputRequest) -> Request {
        let url = self.service_url.join("op-env").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(RootStateAction::CreateOpEnv, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvOutputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvOutputResponse> {
        let http_req = self.encode_create_op_env_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let ret: RootStateCreateOpEnvOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("create op_env from root state success: sid={}", ret.sid);
            Ok(ret)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("create op_env from root state error! {}", e);
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateOutputProcessor for GlobalStateRequestor {
    fn get_category(&self) -> GlobalStateCategory {
        self.category
    }
    
    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootOutputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootOutputResponse> {
        GlobalStateRequestor::get_current_root(&self, req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvOutputRequest,
    ) -> BuckyResult<OpEnvOutputProcessorRef> {
        let op_env_type = req.op_env_type.clone();
        let resp = GlobalStateRequestor::create_op_env(&self, req).await?;

        let requestor = OpEnvRequestor::new(
            self.category.clone(),
            op_env_type,
            resp.sid,
            self.dec_id.clone(),
            self.requestor.clone(),
        );
        Ok(requestor.into_processor())
    }
}

#[derive(Clone)]
pub struct OpEnvRequestor {
    category: GlobalStateCategory,
    op_env_type: ObjectMapOpEnvType,
    sid: u64,

    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl OpEnvRequestor {
    pub fn new(
        category: GlobalStateCategory,
        op_env_type: ObjectMapOpEnvType,
        sid: u64,
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        assert!(sid > 0);

        let addr = requestor.remote_addr();

        let url = format!("http://{}/{}/op-env/", addr, category.as_str());
        let url = Url::parse(&url).unwrap();

        let ret = Self {
            category,
            op_env_type,
            sid,
            dec_id,
            requestor,
            service_url: url,
        };

        ret
    }

    pub fn category(&self) -> &GlobalStateCategory {
        &self.category
    }

    pub fn into_processor(self) -> OpEnvOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> OpEnvOutputProcessorRef {
        self.clone().into_processor()
    }

    fn encode_common_headers(
        &self,
        action: OpEnvAction,
        com_req: &OpEnvOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());

        http_req.insert_header(cyfs_base::CYFS_OP_ENV_ACTION, action.to_string());

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        if com_req.sid > 0 {
            http_req.insert_header(cyfs_base::CYFS_OP_ENV_SID, com_req.sid.to_string());
        } else {
            http_req.insert_header(cyfs_base::CYFS_OP_ENV_SID, self.sid.to_string());
        }
    }

    // load
    // op_env/init/target
    fn encode_load_request(&self, req: &OpEnvLoadOutputRequest) -> Request {
        let url = self.service_url.join("init/target").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::Load, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn load(&self, req: OpEnvLoadOutputRequest) -> BuckyResult<()> {
        if self.op_env_type != ObjectMapOpEnvType::Single {
            let msg = format!("load method only valid for single_op_env! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        let http_req = self.encode_load_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!(
                "load objectmap for single_op_env success: target={}, sid={}",
                req.target, self.sid,
            );
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "load objectmap for single_op_env error! target={}, sid={}, {}",
                req.target, self.sid, e
            );
            Err(e)
        }
    }

    // load_by_path
    fn encode_load_by_path_request(&self, req: &OpEnvLoadByPathOutputRequest) -> Request {
        let url = self.service_url.join("init/path").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::LoadByPath, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathOutputRequest) -> BuckyResult<()> {
        if self.op_env_type != ObjectMapOpEnvType::Single {
            let msg = format!(
                "load_by_path method only valid for single_op_env! sid={}",
                self.sid
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        let http_req = self.encode_load_by_path_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!(
                "load_by_path for single_op_env success: path={}, sid={}",
                req.path, self.sid,
            );
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "load_by_path for single_op_env error! path={}, sid={}, {}",
                req.path, self.sid, e
            );
            Err(e)
        }
    }

    // create_new
    fn encode_create_new_request(&self, req: &OpEnvCreateNewOutputRequest) -> Request {
        let url = self.service_url.join("init/new").unwrap();

        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::CreateNew, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn create_new(&self, req: OpEnvCreateNewOutputRequest) -> BuckyResult<()> {
        let http_req = self.encode_create_new_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("create_new for op_env success: sid={}", self.sid,);
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("create_new for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // lock
    // op_env/{op_env_type}/lock
    fn encode_lock_request(&self, req: &OpEnvLockOutputRequest) -> Request {
        let url = self.service_url.join("lock").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::Lock, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn lock(&self, req: OpEnvLockOutputRequest) -> BuckyResult<()> {
        if self.op_env_type != ObjectMapOpEnvType::Path {
            let msg = format!("lock method only valid for path_op_env! sid={}", self.sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        let http_req = self.encode_lock_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!(
                "lock for path_op_env success: path_list={:?}, sid={}",
                req.path_list, self.sid,
            );
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("lock for path_op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // get_current_root
    fn encode_get_current_root_request(&self, req: &OpEnvGetCurrentRootOutputRequest) -> Request {
        let url = self.service_url.join("root").unwrap();
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(OpEnvAction::GetCurrentRoot, &req.common, &mut http_req);

        http_req
    }

    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootOutputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootOutputResponse> {
        let http_req = self.encode_get_current_root_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: OpEnvGetCurrentRootOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;

            info!("get_current_root for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("get_current_root for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // commit
    fn encode_commit_request(&self, req: &OpEnvCommitOutputRequest) -> Request {
        let url = self.service_url.join("transaction").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::Commit, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn commit(
        &self,
        req: OpEnvCommitOutputRequest,
    ) -> BuckyResult<OpEnvCommitOutputResponse> {
        let http_req = self.encode_commit_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: OpEnvCommitOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;

            info!("commit for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("commit for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // abort
    fn encode_abort_request(&self, req: &OpEnvAbortOutputRequest) -> Request {
        let url = self.service_url.join("transaction").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(OpEnvAction::Abort, &req.common, &mut http_req);

        http_req.set_body(req.encode_string());

        http_req
    }

    async fn abort(&self, req: OpEnvAbortOutputRequest) -> BuckyResult<()> {
        let http_req = self.encode_abort_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            info!("abort for op_env success: sid={}", self.sid,);
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("abort for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // metadata
    fn encode_metadata_request(&self, req: &OpEnvMetadataOutputRequest) -> Request {
        let url = self.service_url.join("metadata").unwrap();
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(OpEnvAction::Metadata, &req.common, &mut http_req);
        RequestorHelper::encode_opt_header_with_encoding(
            &mut http_req,
            cyfs_base::CYFS_OP_ENV_PATH,
            req.path.as_deref(),
        );

        http_req
    }

    async fn metadata(
        &self,
        req: OpEnvMetadataOutputRequest,
    ) -> BuckyResult<OpEnvMetadataOutputResponse> {
        let http_req = self.encode_metadata_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: OpEnvMetadataOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!(
                "get metadata of op_env success: sid={}, resp={:?}",
                self.sid, resp
            );
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("get metadata of op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // get_by_key
    fn encode_get_by_key_request(&self, req: &OpEnvGetByKeyOutputRequest) -> Request {
        let url = self.service_url.join("map").unwrap();
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(OpEnvAction::GetByKey, &req.common, &mut http_req);

        RequestorHelper::encode_opt_header_with_encoding(
            &mut http_req,
            cyfs_base::CYFS_OP_ENV_PATH,
            req.path.as_deref(),
        );
        RequestorHelper::encode_header_with_encoding(
            &mut http_req,
            cyfs_base::CYFS_OP_ENV_KEY,
            &req.key,
        );

        // http_req.set_body(req.encode_string());

        http_req
    }

    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyOutputRequest,
    ) -> BuckyResult<OpEnvGetByKeyOutputResponse> {
        let http_req = self.encode_get_by_key_request(&req);
        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp: OpEnvGetByKeyOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;

            info!("get_by_key for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("get_by_key for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // insert_with_key
    fn encode_insert_with_key_request(&self, req: &OpEnvInsertWithKeyOutputRequest) -> Request {
        let url = self.service_url.join("map").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::InsertWithKey, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }
    async fn insert_with_key(&self, req: OpEnvInsertWithKeyOutputRequest) -> BuckyResult<()> {
        let http_req = self.encode_insert_with_key_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            info!("insert_with_key for op_env success: sid={}", self.sid,);
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("insert_with_key for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // set_with_key
    fn encode_set_with_key_request(&self, req: &OpEnvSetWithKeyOutputRequest) -> Request {
        let url = self.service_url.join("map").unwrap();
        let mut http_req = Request::new(Method::Put, url);
        self.encode_common_headers(OpEnvAction::SetWithKey, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyOutputResponse> {
        let http_req = self.encode_set_with_key_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvSetWithKeyOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("set_with_key for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("set_with_key for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // remove_with_key
    fn encode_remove_with_key_request(&self, req: &OpEnvRemoveWithKeyOutputRequest) -> Request {
        let url = self.service_url.join("map").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(OpEnvAction::RemoveWithKey, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyOutputResponse> {
        let http_req = self.encode_remove_with_key_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvRemoveWithKeyOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("remove_with_key for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("remove_with_key for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // contains
    fn encode_contains_request(&self, req: &OpEnvContainsOutputRequest) -> Request {
        let url = self.service_url.join("set").unwrap();
        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(OpEnvAction::Contains, &req.common, &mut http_req);

        RequestorHelper::encode_opt_header_with_encoding(
            &mut http_req,
            cyfs_base::CYFS_OP_ENV_PATH,
            req.path.as_deref(),
        );
        RequestorHelper::encode_header(&mut http_req, cyfs_base::CYFS_OP_ENV_VALUE, &req.value);

        // http_req.set_body(req.encode_string());
        http_req
    }

    async fn contains(
        &self,
        req: OpEnvContainsOutputRequest,
    ) -> BuckyResult<OpEnvContainsOutputResponse> {
        let http_req = self.encode_contains_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvContainsOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("contains for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("contains for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // insert
    fn encode_insert_request(&self, req: &OpEnvInsertOutputRequest) -> Request {
        let url = self.service_url.join("set").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::Insert, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }

    async fn insert(
        &self,
        req: OpEnvInsertOutputRequest,
    ) -> BuckyResult<OpEnvInsertOutputResponse> {
        let http_req = self.encode_insert_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvInsertOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("insert for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("insert for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // remove
    fn encode_remove_request(&self, req: &OpEnvRemoveOutputRequest) -> Request {
        let url = self.service_url.join("set").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(OpEnvAction::Remove, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }

    async fn remove(
        &self,
        req: OpEnvRemoveOutputRequest,
    ) -> BuckyResult<OpEnvRemoveOutputResponse> {
        let http_req = self.encode_remove_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvRemoveOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("remove for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("remove for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // next
    fn encode_next_request(&self, req: &OpEnvNextOutputRequest) -> Request {
        let url = self.service_url.join("iterator").unwrap();
        let mut http_req = Request::new(Method::Post, url);
        self.encode_common_headers(OpEnvAction::Next, &req.common, &mut http_req);
        http_req.set_body(req.encode_string());
        http_req
    }

    async fn next(&self, req: OpEnvNextOutputRequest) -> BuckyResult<OpEnvNextOutputResponse> {
        let http_req = self.encode_next_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvNextOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("next for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("next for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // reset
    fn encode_reset_request(&self, req: &OpEnvResetOutputRequest) -> Request {
        let url = self.service_url.join("iterator").unwrap();
        let mut http_req = Request::new(Method::Delete, url);
        self.encode_common_headers(OpEnvAction::Reset, &req.common, &mut http_req);
        http_req
    }

    async fn reset(&self, req: OpEnvResetOutputRequest) -> BuckyResult<()> {
        let http_req = self.encode_reset_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            info!("reset for op_env success: sid={}", self.sid,);
            Ok(())
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("reset for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }

    // list
    fn encode_list_request(&self, req: &OpEnvListOutputRequest) -> Request {
        let url = self.service_url.join("list").unwrap();
        let mut http_req = Request::new(Method::Get, url);

        self.encode_common_headers(OpEnvAction::List, &req.common, &mut http_req);
        RequestorHelper::encode_opt_header_with_encoding(&mut http_req, cyfs_base::CYFS_OP_ENV_PATH, req.path.as_deref());

        http_req
    }

    async fn list(&self, req: OpEnvListOutputRequest) -> BuckyResult<OpEnvListOutputResponse> {
        let http_req = self.encode_list_request(&req);
        let mut resp = self.requestor.request(http_req).await?;
        if resp.status().is_success() {
            let resp: OpEnvListOutputResponse =
                RequestorHelper::decode_json_body(&mut resp).await?;
            info!("list for op_env success: sid={}", self.sid,);
            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!("list for op_env error! sid={}, {}", self.sid, e);
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl OpEnvOutputProcessor for OpEnvRequestor {
    fn get_sid(&self) -> u64 {
        self.sid
    }

    async fn load(&self, req: OpEnvLoadOutputRequest) -> BuckyResult<()> {
        Self::load(&self, req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathOutputRequest) -> BuckyResult<()> {
        Self::load_by_path(&self, req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewOutputRequest) -> BuckyResult<()> {
        Self::create_new(&self, req).await
    }

    async fn lock(&self, req: OpEnvLockOutputRequest) -> BuckyResult<()> {
        Self::lock(&self, req).await
    }

    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootOutputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootOutputResponse> {
        Self::get_current_root(&self, req).await
    }

    async fn commit(
        &self,
        req: OpEnvCommitOutputRequest,
    ) -> BuckyResult<OpEnvCommitOutputResponse> {
        Self::commit(&self, req).await
    }
    async fn abort(&self, req: OpEnvAbortOutputRequest) -> BuckyResult<()> {
        Self::abort(&self, req).await
    }

    async fn metadata(
        &self,
        req: OpEnvMetadataOutputRequest,
    ) -> BuckyResult<OpEnvMetadataOutputResponse> {
        Self::metadata(&self, req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyOutputRequest,
    ) -> BuckyResult<OpEnvGetByKeyOutputResponse> {
        Self::get_by_key(&self, req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyOutputRequest) -> BuckyResult<()> {
        Self::insert_with_key(&self, req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyOutputResponse> {
        Self::set_with_key(&self, req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyOutputResponse> {
        Self::remove_with_key(&self, req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsOutputRequest,
    ) -> BuckyResult<OpEnvContainsOutputResponse> {
        Self::contains(&self, req).await
    }

    async fn insert(
        &self,
        req: OpEnvInsertOutputRequest,
    ) -> BuckyResult<OpEnvInsertOutputResponse> {
        Self::insert(&self, req).await
    }

    async fn remove(
        &self,
        req: OpEnvRemoveOutputRequest,
    ) -> BuckyResult<OpEnvRemoveOutputResponse> {
        Self::remove(&self, req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextOutputRequest) -> BuckyResult<OpEnvNextOutputResponse> {
        Self::next(&self, req).await
    }

    async fn reset(&self, req: OpEnvResetOutputRequest) -> BuckyResult<()> {
        Self::reset(&self, req).await
    }

    async fn list(&self, req: OpEnvListOutputRequest) -> BuckyResult<OpEnvListOutputResponse> {
        Self::list(&self, req).await
    }
}

#[derive(Clone)]
pub struct GlobalStateAccessRequestor {
    category: GlobalStateCategory,
    dec_id: Option<SharedObjectStackDecID>,
    requestor: HttpRequestorRef,
    service_url: Url,
}

impl GlobalStateAccessRequestor {
    pub fn new_default_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
    ) -> Self {
        let service_addr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT);
        Self::new_tcp(category, dec_id, &service_addr)
    }

    pub fn new_tcp(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        service_addr: &str,
    ) -> Self {
        let tcp_requestor = TcpHttpRequestor::new(service_addr);
        Self::new(category, dec_id, Arc::new(Box::new(tcp_requestor)))
    }

    pub fn new_root_state(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::RootState, dec_id, requestor)
    }

    pub fn new_local_cache(
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        Self::new(GlobalStateCategory::LocalCache, dec_id, requestor)
    }

    pub fn new(
        category: GlobalStateCategory,
        dec_id: Option<SharedObjectStackDecID>,
        requestor: HttpRequestorRef,
    ) -> Self {
        let addr = requestor.remote_addr();

        let url = format!("http://{}/{}/", addr, category.as_str());
        let url = Url::parse(&url).unwrap();

        let ret = Self {
            category,
            dec_id,
            requestor,
            service_url: url,
        };

        ret
    }

    pub fn category(&self) -> &GlobalStateCategory {
        &self.category
    }

    pub fn into_processor(self) -> GlobalStateAccessOutputProcessorRef {
        Arc::new(Box::new(self))
    }

    pub fn clone_processor(&self) -> GlobalStateAccessOutputProcessorRef {
        self.clone().into_processor()
    }

    // TODO: 目前和request的body部分编码一部分冗余的信息
    fn encode_common_headers(
        &self,
        com_req: &RootStateOutputRequestCommon,
        http_req: &mut Request,
    ) {
        if let Some(dec_id) = &com_req.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        } else if let Some(dec_id) = &self.dec_id {
            if let Some(dec_id) = dec_id.get() {
                http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
            }
        }

        if let Some(target) = &com_req.target {
            http_req.insert_header(cyfs_base::CYFS_TARGET, target.to_string());
        }

        http_req.insert_header(cyfs_base::CYFS_FLAGS, com_req.flags.to_string());
    }

    ////// access methods

    fn gen_url(&self, inner_path: &str) -> Url {
        self.service_url
            .join(&inner_path.trim_start_matches("/"))
            .unwrap()
    }

    // get_object_by_path
    fn encode_get_object_by_path_request(
        &self,
        req: &RootStateAccessGetObjectByPathOutputRequest,
    ) -> Request {
        let url = self.gen_url(&req.inner_path);

        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn decode_get_object_by_path_response(
        resp: &mut Response,
    ) -> BuckyResult<RootStateAccessGetObjectByPathOutputResponse> {
        let object = NONRequestorHelper::decode_get_object_response(resp).await?;
        let root = RequestorHelper::decode_header(resp, cyfs_base::CYFS_ROOT)?;
        let revision = RequestorHelper::decode_header(resp, cyfs_base::CYFS_REVISION)?;

        Ok(RootStateAccessGetObjectByPathOutputResponse {
            object,
            root,
            revision,
        })
    }

    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathOutputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathOutputResponse> {
        debug!("access get_object_by_path: {}", req);

        let http_req = self.encode_get_object_by_path_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let info = Self::decode_get_object_by_path_response(&mut resp).await?;
            info!(
                "get_object_by_path from global state success: category={}, inner_path={}, {}",
                self.category, req.inner_path, info
            );
            Ok(info)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "get_object_by_path from global state error: category={}, inner_path={}, {}",
                self.category, req.inner_path, e
            );
            Err(e)
        }
    }

    // list
    fn encode_list_request(&self, req: &RootStateAccessListOutputRequest) -> Request {
        let mut url = self.gen_url(&req.inner_path);
        debug!("list url: {}, {}", url, req.inner_path);

        {
            let mut querys = url.query_pairs_mut();
            querys.append_pair("action", &RootStateAccessAction::List.to_string());

            if let Some(page_index) = &req.page_index {
                querys.append_pair("page_index", &page_index.to_string());
            }

            if let Some(page_size) = &req.page_size {
                querys.append_pair("page_size", &page_size.to_string());
            }
        }

        let mut http_req = Request::new(Method::Get, url);
        self.encode_common_headers(&req.common, &mut http_req);

        http_req
    }

    pub async fn decode_list_response(
        resp: &mut Response,
    ) -> BuckyResult<RootStateAccessListOutputResponse> {
        let list = RequestorHelper::decode_json_body(resp).await?;
        let root = RequestorHelper::decode_header(resp, cyfs_base::CYFS_ROOT)?;
        let revision = RequestorHelper::decode_header(resp, cyfs_base::CYFS_REVISION)?;

        Ok(RootStateAccessListOutputResponse {
            list,
            root,
            revision,
        })
    }

    async fn list(
        &self,
        req: RootStateAccessListOutputRequest,
    ) -> BuckyResult<RootStateAccessListOutputResponse> {
        debug!("access list: {}", req);

        let http_req = self.encode_list_request(&req);

        let mut resp = self.requestor.request(http_req).await?;

        if resp.status().is_success() {
            let resp = Self::decode_list_response(&mut resp).await?;

            info!(
                "list from global state success: category={}, req={}, count={}, root={}, revision={}",
                self.category,
                req,
                resp.list.len(),
                resp.root,
                resp.revision,
            );

            Ok(resp)
        } else {
            let e = RequestorHelper::error_from_resp(&mut resp).await;
            error!(
                "list from global state error: category={}, req={}, {}",
                self.category, req, e
            );
            Err(e)
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessOutputProcessor for GlobalStateAccessRequestor {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathOutputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathOutputResponse> {
        Self::get_object_by_path(self, req).await
    }

    async fn list(
        &self,
        req: RootStateAccessListOutputRequest,
    ) -> BuckyResult<RootStateAccessListOutputResponse> {
        Self::list(self, req).await
    }
}

#[test]
fn test_url() {
    let url = Url::parse("http://www.cyfs.com").unwrap();
    let mut http_req = Request::new(Method::Get, url);

    RequestorHelper::encode_header(&mut http_req, &"Content-Type", &"text/html; charset=utf-8");

    let value = "新建文件夹";

    RequestorHelper::encode_header_with_encoding(
        &mut http_req,
        cyfs_base::CYFS_OP_ENV_PATH,
        &value,
    );
    let header =
        RequestorHelper::decode_header_with_utf8_decoding(&http_req, cyfs_base::CYFS_OP_ENV_PATH)
            .unwrap();
    assert_eq!(header, value);

    let value = "/article/standby";
    RequestorHelper::encode_header_with_encoding(
        &mut http_req,
        cyfs_base::CYFS_OP_ENV_PATH,
        &value,
    );
    let header =
        RequestorHelper::decode_header_with_utf8_decoding(&http_req, cyfs_base::CYFS_OP_ENV_PATH)
            .unwrap();
    assert_eq!(header, value);

    let value = RequestorHelper::decode_utf8("test", "%2Farticle%2Fstandby").unwrap();
    println!("{}", value);

    let v1 = "%2Fa%2Fb%2F%E6%88%91%E7%9A%84%2F%20%2F**";
    let v2 = "/a/b/%E6%88%91%E7%9A%84/%20/**";
    let value = RequestorHelper::decode_utf8("test", v1).unwrap();
    println!("{}", value);
    let value2 = RequestorHelper::decode_utf8("test", v2).unwrap();
    println!("{}", value);
    assert_eq!(value, value2);

    let url = format!("http://{}/{}/", "addr", "category/新建文件夹");
    let url = Url::parse(&url).unwrap();
    let inner_path = "/test/it";
    let url = url.join(&inner_path.trim_start_matches("/")).unwrap();

    println!("{}", url);
}
