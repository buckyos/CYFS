use crate::ndn::*;
use crate::ndn_api::NDNForwardObjectData;
use crate::root_state::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(super) struct GlobalStateAccessWithNDNInputProcessor {
    next: GlobalStateAccessInputProcessorRef,
    ndn: NDNInputProcessorRef,
    device_id: DeviceId,
}

impl GlobalStateAccessWithNDNInputProcessor {
    pub fn new(
        next: GlobalStateAccessInputProcessorRef,
        ndn: NDNInputProcessorRef,
        device_id: DeviceId,
    ) -> GlobalStateAccessInputProcessorRef {
        let ret = Self {
            next,
            ndn,
            device_id,
        };

        Arc::new(Box::new(ret))
    }

    async fn get_data(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
        mut file_resp: NONGetObjectInputResponse,
    ) -> BuckyResult<NDNGetDataInputResponse> {

        let object = file_resp.object.take_object();

        // select the ndn request target, first file's owner, then the req.common.target as the same with access request
        let target = match object.owner() {
            Some(target) => Some(target.to_owned()),
            None => {
                req.common.target
            }
        };

        // the file object shold been set prehead!
        let object = match Arc::try_unwrap(object) {
            Ok(v) => v,
            Err(_) => unreachable!(),
        };
        let file = match object {
            AnyNamedObject::Standard(StandardObject::File(file)) => file,
            _ => unreachable!(),
        };

        let user_data = NDNForwardObjectData { 
            file,
            file_id: file_resp.object.object_id,
        };
 

        let ndn_req = NDNGetDataInputRequest {
            common: NDNInputRequestCommon {
                req_path: Some(req.inner_path),
                dec_id: req.common.dec_id,

                source: self.device_id.clone(),
                protocol: req.common.protocol,
                level: NDNAPILevel::Router,

                referer_object: vec![],
                target,
                flags: 0,
                user_data: Some(user_data.to_any()),
            },
            object_id: file_resp.object.object_id,
            data_type: NDNDataType::Mem,

            // FIXME 支持range
            range: None,
            
            inner_path: None,
        };

        self.ndn.get_data(ndn_req).await
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessInputProcessor for GlobalStateAccessWithNDNInputProcessor {
    async fn get_object_by_path(
        &self,
        mut req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        let mode = req.mode;

        // ndn only been processed on the origin stack
        req.mode = RootStateAccessGetMode::Object;

        let ndn_req = match mode {
            RootStateAccessGetMode::Object => None,
            RootStateAccessGetMode::Data | RootStateAccessGetMode::Default => Some(req.clone()),
        };

        let resp = self.next.get_object_by_path(req).await?;
        assert!(resp.object.is_some());
        assert!(resp.data.is_none());

        if mode == RootStateAccessGetMode::Object {
            return Ok(resp);
        }

        let req = ndn_req.unwrap();

        // only file object is valid for ndn
        match resp
            .object
            .as_ref()
            .unwrap()
            .object
            .object_id
            .obj_type_code()
        {
            ObjectTypeCode::File => {}
            _ => {
                if mode == RootStateAccessGetMode::Default {
                    return Ok(resp);
                }

                let object = resp.object.unwrap();
                let msg = format!("rpath access's data mode only support for FileObject! path={}, obj={}, type={:?}",
                    req.inner_path, object.object.object_id, object.object.object_id.obj_type_code());
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        }

        let object = resp.object.unwrap();

        let data = self.get_data(req, object).await?;
        Ok(RootStateAccessGetObjectByPathInputResponse {
            data: Some(data),
            object: None,
        })
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        self.next.list(req).await
    }
}