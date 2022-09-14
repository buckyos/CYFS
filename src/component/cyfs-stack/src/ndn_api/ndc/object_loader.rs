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
    pub fn new(
        non_processor: NONInputProcessorRef,
    ) -> Self {
        Self {
            non_processor,
        }
    }


    // get_file存在两种形式:
    // 1. file_id + referer_object
    // 2. dir_id + inner_path + referer_object
    // 需要把referer_object合并到req_path里面
    pub async fn get_file_object(&self, req: &NDNGetDataInputRequest, target: Option<&DeviceId>) -> BuckyResult<(ObjectId, File)> {
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

    // 获取root object，必须是file or dir
    pub async fn get_root_object(&self, req: &NDNGetDataInputRequest, target: Option<&DeviceId>) -> BuckyResult<NONObjectInfo> {
        if req.common.referer_object.is_empty() {
            self.get_root_object_with_referer(&req, None, target).await
        } else {
            let mut error = None;
            for referer_object in &req.common.referer_object {
                match self
                    .get_root_object_with_referer(&req, Some(referer_object), target)
                    .await
                {
                    Ok(file) => return Ok(file),
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
        target: Option<&DeviceId>
    ) -> BuckyResult<(ObjectId, File)> {
        let resp = self.get_object_with_referer(req, referer_object, target).await?;

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

    async fn get_root_object_with_referer(
        &self,
        req: &NDNGetDataInputRequest,
        referer_object: Option<&NDNDataRefererObject>,
        target: Option<&DeviceId>
    ) -> BuckyResult<NONObjectInfo> {
        let resp = self.get_object_with_referer(req, referer_object, target).await?;

        Ok(resp.object)
    }

    async fn get_object_with_referer(
        &self,
        req: &NDNGetDataInputRequest,
        referer_object: Option<&NDNDataRefererObject>,
        target: Option<&DeviceId>
    ) -> BuckyResult<NONGetObjectInputResponse> {
        // 重新推导req_path
        let req_path = match referer_object {
            Some(referer_object) => {
                let path = AclResource::join(
                    &req.common.req_path,
                    &Some(referer_object.object_id),
                    &referer_object.inner_path,
                );
                Some(path)
            }
            None => req.common.req_path.clone(),
        };
        let get_req = NONGetObjectInputRequest {
            common: NONInputRequestCommon {
                req_path,
                source: req.common.source.clone(),
                level: req.common.level.clone().into(),
                target: target.map(|v| v.object_id().to_owned()),
                flags: req.common.flags,
            },

            object_id: req.object_id.clone(),
            inner_path: req.inner_path.clone(),
        };

        let resp = self.non_processor.get_object(get_req).await?;
        Ok(resp)
    }
}
