use super::def::*;
use super::request::*;
use crate::ndn::NDNInputProcessorRef;
use crate::ndn_api::NDNForwardObjectData;
use crate::non::NONInputProcessorRef;
use crate::resolver::OodResolver;
use crate::root_state::GlobalStateAccessInputProcessorRef;
use cyfs_base::*;
use cyfs_lib::*;

pub struct FrontService {
    non: NONInputProcessorRef,
    ndn: NDNInputProcessorRef,

    global_state_processor: GlobalStateAccessInputProcessorRef,
    ood_resolver: OodResolver,
}

impl FrontService {
    pub async fn process_o_request(&self, req: FrontORequest) -> BuckyResult<FrontOResponse> {
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
            None
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
                Some(req.target[0])
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

    async fn resolve_target_from_file(&self, object: &NONObjectInfo) -> BuckyResult<Vec<DeviceId>> {
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
                        if !targets.iter().any(|v| *v == device_id) {
                            targets.push(device_id);
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

        self.global_state_processor
            .get_object_by_path(state_req)
            .await
    }
}
