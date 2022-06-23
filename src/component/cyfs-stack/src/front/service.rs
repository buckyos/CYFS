use super::def::*;
use super::request::*;
use crate::app::AppInstallStatus;
use crate::app::AppService;
use crate::ndn::NDNInputProcessorRef;
use crate::ndn_api::NDNForwardObjectData;
use crate::non::NONInputProcessorRef;
use crate::resolver::OodResolver;
use crate::root_state::GlobalStateAccessInputProcessorRef;
use cyfs_base::*;
use cyfs_lib::*;


pub(crate) struct FrontService {
    non: NONInputProcessorRef,
    ndn: NDNInputProcessorRef,

    root_state: GlobalStateAccessInputProcessorRef,
    local_cache: GlobalStateAccessInputProcessorRef,

    app: AppService,

    ood_resolver: OodResolver,
}

impl FrontService {
    pub fn new(
        non: NONInputProcessorRef,
        ndn: NDNInputProcessorRef,
        root_state: GlobalStateAccessInputProcessorRef,
        local_cache: GlobalStateAccessInputProcessorRef,
        app: AppService,
        ood_resolver: OodResolver,
    ) -> Self {
        Self {
            non,
            ndn,
            root_state,
            local_cache,
            app,
            ood_resolver,
        }
    }

    pub async fn process_o_request(&self, req: FrontORequest) -> BuckyResult<FrontOResponse> {
        info!("will process o request: {:?}", req);

        let resp = match req.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                // verify the mode
                let mode = Self::select_mode(&req.mode, &req.object_id)?;
                assert_eq!(mode, FrontRequestGetMode::Data);

                let ndn_req = FrontNDNRequest::new_o_chunk(req);
                let resp = self.process_get_chunk(ndn_req).await?;

                FrontOResponse {
                    object: None,
                    data: Some(resp),
                }
            }
            _ => {
                let non_resp = self.process_get_object(req.clone()).await?;

                // decide the mode
                let mode = Self::select_mode(&req.mode, &non_resp.object.object_id)?;

                match mode {
                    FrontRequestGetMode::Object => FrontOResponse {
                        object: Some(non_resp),
                        data: None,
                    },
                    FrontRequestGetMode::Data => {
                        let ndn_req = FrontNDNRequest::new_o_file(req, non_resp.object.clone());
                        let ndn_resp = self.process_get_file(ndn_req).await?;

                        FrontOResponse {
                            object: Some(non_resp),
                            data: Some(ndn_resp),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        };

        Ok(resp)
    }

    async fn process_get_object(
        &self,
        req: FrontORequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let target = if req.target.len() > 0 {
            Some(req.target[0])
        } else {
            if let Ok(list) = self.resolve_target_from_object_id(&req.object_id).await {
                if list.len() > 0 {
                    Some(list[0])
                } else {
                    None
                }
            } else {
                None
            }
        };

        let common = NONInputRequestCommon {
            req_path: None,
            dec_id: req.dec_id,
            source: req.source,
            protocol: req.protocol,
            level: NONAPILevel::Router,
            target,
            flags: req.flags,
        };

        let non_req = NONGetObjectInputRequest {
            common,
            object_id: req.object_id,
            inner_path: req.inner_path,
        };

        self.non.get_object(non_req).await
    }

    async fn process_get_chunk(
        &self,
        req: FrontNDNRequest,
    ) -> BuckyResult<NDNGetDataInputResponse> {
        assert_eq!(req.object.object_id.obj_type_code(), ObjectTypeCode::Chunk);

        let target = if req.target.len() > 0 {
            Some(req.target[0])
        } else {
            None
        };

        let common = NDNInputRequestCommon {
            req_path: None,
            dec_id: req.dec_id,
            source: req.source,
            protocol: req.protocol,
            level: NDNAPILevel::Router,
            referer_object: vec![],
            target,
            flags: req.flags,
            user_data: None,
        };

        let ndn_req = NDNGetDataInputRequest {
            common,
            object_id: req.object.object_id,
            data_type: NDNDataType::Mem,
            range: None,
            inner_path: None,
        };

        self.ndn.get_data(ndn_req).await
    }

    async fn process_get_file(&self, req: FrontNDNRequest) -> BuckyResult<NDNGetDataInputResponse> {
        assert_eq!(req.object.object_id.obj_type_code(), ObjectTypeCode::File);

        let file: AnyNamedObject = req.object.object.as_ref().unwrap().clone().into();
        let file = file.into_file();

        let data = NDNForwardObjectData {
            file,
            file_id: req.object.object_id.clone(),
        };

        // FIXME how to decide the file target? and multi target support
        let target = if req.target.len() > 0 {
            Some(req.target[0])
        } else {
            let targets = self.resolve_target_from_file(&req.object).await?;
            if targets.len() > 0 {
                Some(targets[0])
            } else {
                None
            }
        };

        let common = NDNInputRequestCommon {
            req_path: None,
            dec_id: req.dec_id,
            source: req.source,
            protocol: req.protocol,
            level: NDNAPILevel::Router,
            referer_object: vec![],
            target,
            flags: req.flags,
            user_data: Some(data.to_any()),
        };

        let req = NDNGetDataInputRequest {
            common,
            object_id: req.object.object_id,
            data_type: NDNDataType::Mem,
            range: None,
            inner_path: None,
        };

        self.ndn.get_data(req).await
    }

    async fn resolve_target_from_object_id(
        &self,
        object_id: &ObjectId,
    ) -> BuckyResult<Vec<ObjectId>> {
        let mut sources = vec![];
        match self.ood_resolver.resolve_ood(object_id, None).await {
            Ok(list) => {
                if list.is_empty() {
                    info!(
                        "get target from path root seg but not found! seg={}",
                        object_id,
                    );
                } else {
                    info!(
                        "get target from path root seg success! seg={}, sources={:?}",
                        object_id, list
                    );

                    list.into_iter().for_each(|device_id| {
                        // 这里需要列表去重
                        let id = device_id.into();
                        if !sources.iter().any(|v| *v == id) {
                            sources.push(id);
                        }
                    });
                }

                Ok(sources)
            }
            Err(e) => {
                error!(
                    "get target from path root seg failed! id={}, {}",
                    object_id, e
                );
                Err(e)
            }
        }
    }

    async fn resolve_target_from_file(&self, object: &NONObjectInfo) -> BuckyResult<Vec<ObjectId>> {
        let mut targets = vec![];
        match self
            .ood_resolver
            .get_ood_by_object(
                object.object_id.clone(),
                None,
                object.object.as_ref().unwrap().clone(),
            )
            .await
        {
            Ok(list) => {
                if list.is_empty() {
                    info!(
                        "get target from file object but not found! file={}",
                        object.object_id,
                    );
                } else {
                    info!(
                        "get target from file object success! file={}, targets={:?}",
                        object.object_id, list
                    );

                    list.into_iter().for_each(|device_id| {
                        // 这里需要列表去重
                        let id = device_id.into();
                        if !targets.iter().any(|v| *v == id) {
                            targets.push(id);
                        }
                    });
                }

                Ok(targets)
            }
            Err(e) => {
                error!(
                    "get target from file object failed! file={}, {}",
                    object.object_id, e
                );
                Err(e)
            }
        }
    }

    fn select_mode(
        mode: &FrontRequestGetMode,
        object_id: &ObjectId,
    ) -> BuckyResult<FrontRequestGetMode> {
        let mode = match mode {
            FrontRequestGetMode::Object => {
                if object_id.obj_type_code() == ObjectTypeCode::Chunk {
                    let msg = format!("chunk not support object mode! chunk={}", object_id,);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
                }

                FrontRequestGetMode::Object
            }
            FrontRequestGetMode::Data => {
                if !Self::is_data_mode_valid(object_id) {
                    let msg = format!(
                        "object not support data mode! object={}, type={:?}",
                        object_id,
                        object_id.obj_type_code(),
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
                }

                FrontRequestGetMode::Data
            }
            FrontRequestGetMode::Default => {
                if Self::is_data_mode_valid(object_id) {
                    FrontRequestGetMode::Data
                } else {
                    FrontRequestGetMode::Object
                }
            }
        };

        Ok(mode)
    }

    fn is_data_mode_valid(object_id: &ObjectId) -> bool {
        match object_id.obj_type_code() {
            ObjectTypeCode::File | ObjectTypeCode::Chunk => true,
            _ => false,
        }
    }

    pub async fn process_r_request(&self, req: FrontRRequest) -> BuckyResult<FrontRResponse> {
        info!("will process r request: {:?}", req);

        let state_resp = self.process_global_state_request(req.clone()).await?;

        let resp = match state_resp.object.object.object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                // verify the mode
                let mode = Self::select_mode(&req.mode, &state_resp.object.object.object_id)?;
                assert_eq!(mode, FrontRequestGetMode::Data);

                let ndn_req = FrontNDNRequest::new_r_resp(req, state_resp.object.object.clone());
                let resp = self.process_get_chunk(ndn_req).await?;

                FrontRResponse {
                    object: Some(state_resp.object),
                    root: state_resp.root,
                    revision: state_resp.revision,
                    data: Some(resp),
                }
            }
            _ => {
                // decide the mode
                let mode = Self::select_mode(&req.mode, &state_resp.object.object.object_id)?;

                match mode {
                    FrontRequestGetMode::Object => FrontRResponse {
                        object: Some(state_resp.object),
                        root: state_resp.root,
                        revision: state_resp.revision,
                        data: None,
                    },
                    FrontRequestGetMode::Data => {
                        let ndn_req =
                            FrontNDNRequest::new_r_resp(req, state_resp.object.object.clone());
                        let ndn_resp = self.process_get_file(ndn_req).await?;

                        FrontRResponse {
                            object: Some(state_resp.object),
                            root: state_resp.root,
                            revision: state_resp.revision,
                            data: Some(ndn_resp),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        };

        Ok(resp)
    }

    async fn process_global_state_request(
        &self,
        req: FrontRRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        let common = RootStateInputRequestCommon {
            dec_id: req.dec_id,
            source: req.source,
            protocol: req.protocol,
            target: req.target,
            flags: req.flags,
        };

        let state_req = RootStateAccessGetObjectByPathInputRequest {
            common,
            inner_path: req.inner_path.unwrap_or("".to_owned()),
        };

        let processor = match req.category {
            GlobalStateCategory::RootState => &self.root_state,
            GlobalStateCategory::LocalCache => &self.local_cache,
        };

        processor.get_object_by_path(state_req).await
    }

    pub async fn process_a_request(&self, req: FrontARequest) -> BuckyResult<FrontAResponse> {
        info!("will process a request: {:?}", req);

        let target = match req.target {
            Some(id) => vec![id],
            None => vec![],
        };

        let resp = match req.goal {
            FrontARequestGoal::Web(web_req) => {
                let ret = self.app.get_app_web_dir(&req.dec, &web_req.version).await?;
                match ret {
                    AppInstallStatus::Installed((dec_id, dir_id)) => {
                        let o_req = FrontORequest {
                            protocol: req.protocol,
                            source: req.source,

                            target,

                            dec_id: Some(dec_id),
                            object_id: dir_id,
                            inner_path: web_req.inner_path,

                            mode: req.mode,
                            format: req.format,

                            flags: req.flags,
                        };

                        let url = self.gen_o_redirect_url(&o_req);
                        FrontAResponse::Redirect(url)
                        //let o_resp = self.process_o_request(o_req).await?;
                        //FrontAResponse::Response(o_resp)
                    }
                    AppInstallStatus::NotInstalled(dec) => {
                        let dec_id = dec.as_dec_id().or(req.dec.as_dec_id());
                        let name = dec.as_name().or(req.dec.as_name());
                        
                        let url = self.gen_app_not_installed_redirect_url(
                            dec_id,
                            name,
                            Some(&web_req.version),
                        );
                        FrontAResponse::Redirect(url)
                    }
                }
            }
            FrontARequestGoal::LocalStatus => {
                let ret = self.app.get_app_local_status(&req.dec).await?;
                match ret {
                    AppInstallStatus::Installed((dec_id, local_status_id)) => {
                        let o_req = FrontORequest {
                            protocol: req.protocol,
                            source: req.source,

                            target,

                            dec_id: Some(dec_id),
                            object_id: local_status_id,
                            inner_path: None,

                            mode: req.mode,
                            format: req.format,

                            flags: req.flags,
                        };

                        let o_resp = self.process_o_request(o_req).await?;

                        FrontAResponse::Response(o_resp)
                    }
                    AppInstallStatus::NotInstalled(dec) => {
                        let dec_id = dec.as_dec_id().or(req.dec.as_dec_id());
                        let name = dec.as_name().or(req.dec.as_name());

                        let url = self.gen_app_not_installed_redirect_url(dec_id, name, None);
                        FrontAResponse::Redirect(url)
                    }
                }
            }
        };

        Ok(resp)
    }

    fn gen_o_redirect_url(
        &self,
        req: &FrontORequest,
    ) -> String {
        let mut parts = vec![];
        if req.target.len() > 0 {
            let targets: Vec<String> = req.target.iter().map(|v| v.to_string()).collect();
            parts.push(targets.join(","));
        }

        parts.push(req.object_id.to_string());

        if let Some(inner_path) = &req.inner_path {
            parts.push(inner_path.trim_start_matches("/").to_owned());
        }

        let url = format!("/o/{}", parts.join("/"));

        let mut querys = vec![];
        if let Some(dec_id) = &req.dec_id {
            querys.push(format!("dec_id={}", dec_id));
        }

        if req.flags != 0 {
            querys.push(format!("flags={}", req.flags));
        }

        if req.mode != FrontRequestGetMode::Default {
            querys.push(format!("mode={}", req.mode.as_str()));
        }

        if req.format != FrontRequestObjectFormat::Default {
            querys.push(format!("format={}", req.format.as_str()));
        }

        let url = if querys.len() > 0 {
            let querys = querys.join("&");
            format!("{}?{}", url, querys)
        } else {
            url
        };
        
        url
    }

    fn gen_app_not_installed_redirect_url(
        &self,
        dec_id: Option<&ObjectId>,
        name: Option<&str>,
        version: Option<&FrontARequestVersion>,
    ) -> String {
        let mut querys = vec![];

        // FIXME distinguish between not installed and version unmatch
        querys.push("error=not_installed".to_owned());

        if let Some(dec_id) = dec_id {
            querys.push(format!("dec_id={}", dec_id));
        }

        if let Some(name) = name {
            let v: String =
                percent_encoding::utf8_percent_encode(name, percent_encoding::NON_ALPHANUMERIC)
                    .collect();

                querys.push(format!("name={}", v));
        }

        if let Some(version) = &version {
            match version {
                FrontARequestVersion::Version(ver) => {
                    querys.push(format!("version={}", ver));
                }
                FrontARequestVersion::DirID(dir_id) => {
                    querys.push(format!("dir_id={}", dir_id));
                }
                FrontARequestVersion::Current => {}
            }
        }

        let url = if querys.len() > 0 {
            let querys = querys.join("&");
            format!("{}?{}", APP_DETAIL_URL, querys)
        } else {
            APP_DETAIL_URL.to_owned()
        };
        
        url
    }
}

const APP_DETAIL_URL: &str = "/static/app/app_detail.html";