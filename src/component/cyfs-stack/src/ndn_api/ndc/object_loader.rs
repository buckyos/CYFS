use std::fmt::format;

use crate::acl::AclResource;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

/*
加载文件数据，分为本地和远程加载
需要分object和data两部分，也分两部分权限校验

A: out-get-object  res=/B/dec_id/req_path/file_id target=B
B: in-get-object res=/dec_id/req_path/file_id source=A
A: out-get-data res=/B/dec_id/req_path/file_id target=B
B: in-get-data res=/dec_id/req_path/file_id source=A

所以关键点是
1. 构造正确的request发起non操作，包括req_path
2. 选择正确的non_processor
*/

// 用以处理ndn请求里面对object的查找
#[derive(Clone)]
pub(crate) struct NDNObjectLoader {
    // 适用的non
    non_processor: NONInputProcessorRef,
}

impl NDNObjectLoader {
    pub fn new(non_processor: NONInputProcessorRef) -> Self {
        Self { non_processor }
    }

    // get_file存在两种形式:
    // 1. 
    // 1. file_id + referer_object
    // 2. dir_id + inner_path + referer_object
    // 需要把referer_object合并到req_path里面
    pub async fn get_file_object(
        &self,
        req: &NDNGetDataInputRequest,
        target: Option<&DeviceId>,
    ) -> BuckyResult<(ObjectId, File)> {
        if req.common.referer_object.is_empty() {
            self.get_file_object_with_referer(&req, None, target).await
        } else {
            let mut error = None;
            for referer_object in &req.common.referer_object {
                match self
                    .get_file_object_with_referer(&req, Some(referer_object), target)
                    .await
                {
                    Ok(ret) => return Ok(ret),
                    Err(e) => error = Some(e),
                }
            }

            Err(error.unwrap())
        }
    }

    async fn get_file_object_with_referer(
        &self,
        req: &NDNGetDataInputRequest,
        referer_object: Option<&NDNDataRefererObject>,
        target: Option<&DeviceId>,
    ) -> BuckyResult<(ObjectId, File)> {
        let resp = self
            .get_object_with_referer(req, referer_object, target)
            .await?;

        // 返回值可能是file or dir，目前只支持file
        let id = resp.object.object_id;
        if id.obj_type_code() != ObjectTypeCode::File {
            let msg = format!(
                "ndn get_file but not file object! object={}, inner_path={:?}, got={}, type={:?}",
                req.object_id,
                req.inner_path,
                id,
                id.obj_type_code(),
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        if let AnyNamedObject::Standard(StandardObject::File(file)) =
            resp.object.object.unwrap().into()
        {
            Ok((id, file))
        } else {
            unreachable!()
        }
    }

    async fn get_object_with_referer(
        &self,
        req: &NDNGetDataInputRequest,
        referer_object: Option<&NDNDataRefererObject>,
        target: Option<&DeviceId>,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let req_object;
        let req_inner_path;

        match req.object_id.obj_type_code() {
            ObjectTypeCode::File => {
                if let Some(referer) = referer_object {
                    if referer.is_inner_path_empty() {
                        let msg = format!(
                            "ndn invalid referer object's inner_path! target={}, referer={}",
                            req.object_id, referer
                        );
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }

                    req_object = referer.object_id.clone();
                    req_inner_path = referer.inner_path.clone();
                } else {
                    req_object = req.object_id.clone();
                    req_inner_path = None;
                }
            }
            ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                if referer_object.is_some() {
                    warn!("ndn target is dir/objectmap but already has referer object! target={}, inner_path={:?}, referer={:?}", 
                        req.object_id, req.inner_path, referer_object);
                }

                req_object = req.object_id.clone();
                req_inner_path = req.inner_path.clone();
            }
            code @ _ => {
                let msg = format!(
                    "support ndn get object request type code: {}, {:?}",
                    req.object_id, code
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        }


        let get_req = NONGetObjectInputRequest {
            common: NONInputRequestCommon {
                req_path: req.common.req_path.clone(),
                source: req.common.source.clone(),
                level: req.common.level.clone().into(),
                target: target.map(|v| v.object_id().to_owned()),
                flags: req.common.flags,
            },

            object_id: req_object,
            inner_path: req_inner_path,
        };

        let resp = self.non_processor.get_object(get_req).await?;
        
        // check if matched
        if req.object_id.obj_type_code() == ObjectTypeCode::File {
            if req.object_id != resp.object.object_id {
                let msg = format!("ndn get object but unmatched! target={}, got={}, referer={:?}", req.object_id, resp.object.object_id, referer_object);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }
        }

        Ok(resp)
    }
}
