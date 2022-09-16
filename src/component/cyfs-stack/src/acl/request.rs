use super::group::*;
use super::manager::AclMatchInstanceRef;
use super::res::AclResource;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_lib::{NDNDataRefererObject, RequestProtocol};

use once_cell::sync::OnceCell;
use std::sync::Arc;

#[async_trait::async_trait]
pub(crate) trait AclRequest: Send + Sync {
    // 来源protocol
    fn protocol(&self) -> &RequestProtocol;

    // 请求动作
    fn action(&self) -> &AclAction;

    // 一个请求可以属于多个资源路径
    async fn resource(&self) -> &Vec<String>;

    // zone内外
    async fn location(&self) -> BuckyResult<&AclGroupLocation>;

    // 所属dec
    fn dec(&self) -> &str;

    // 来源设备/目标设备
    fn device(&self) -> &DeviceId;

    // 目标对象
    fn object_id(&self) -> Option<&ObjectId>;
    async fn object(&self) -> BuckyResult<Option<&Arc<AnyNamedObject>>>;

    // 用以调试诊断的信息
    fn debug_info(&self) -> &str;

    // 用以display
    fn display(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result;

    // 获取handler_request
    async fn handler_req(&self) -> &RouterHandlerAclRequest;
}

pub(crate) struct AclRequestWrapper {
    match_instance: AclMatchInstanceRef,

    // 来源协议
    protocol: RequestProtocol,

    // 动作
    action: AclAction,

    // 所属dec
    dec_id_str: String,

    // 请求路径
    req_path: Option<String>,
    req_path_list: OnceCell<Vec<String>>,

    // group location
    location: OnceCell<BuckyResult<AclGroupLocation>>,

    // 对象内部路径，主要用以dir
    inner_path: Option<String>,

    // 引用对象，主要用以NDN
    pub referer_object: Vec<NDNDataRefererObject>,

    // 来源设备source/目标设备target，本地调用的话，source就是协议栈自身device-id
    // 根据action.direction来确定是source还是target
    device_id: DeviceId,

    // 目标对象
    // 在r路径下支持object_id为空
    object_id: Option<ObjectId>,
    object: OnceCell<BuckyResult<Option<Arc<AnyNamedObject>>>>,

    debug_info: OnceCell<String>,

    // 用以处理handler的request
    handler_req: OnceCell<RouterHandlerAclRequest>,
}

pub(crate) enum AclRequestDevice {
    Source(DeviceId),
    Target(DeviceId),
}

pub(crate) struct AclRequestParams {
    pub protocol: RequestProtocol,

    pub direction: AclDirection,
    pub operation: AclOperation,

    // source/target
    pub device_id: AclRequestDevice,

    pub object_id: Option<ObjectId>,
    pub object: Option<Arc<AnyNamedObject>>,

    pub dec_id: Option<ObjectId>,
    pub req_path: Option<String>,
    pub inner_path: Option<String>,

    // 引用对象
    pub referer_object: Option<Vec<NDNDataRefererObject>>,
}

impl AclRequestWrapper {
    pub fn new_from_params(match_instance: AclMatchInstanceRef, param: AclRequestParams) -> Self {
        let device_id = match param.device_id {
            AclRequestDevice::Source(device_id) => device_id,
            AclRequestDevice::Target(device_id) => device_id,
        };
        let referer_object = param.referer_object.unwrap_or_else(|| vec![]);

        let object = OnceCell::new();
        if let Some(o) = param.object {
            let _ = object.set(Ok(Some(o)));
        }

        Self {
            match_instance,

            protocol: param.protocol,
            action: AclAction::new(param.direction, param.operation),

            dec_id_str: Self::calc_dec(param.dec_id.as_ref()),

            req_path: param.req_path,
            req_path_list: OnceCell::new(),

            location: OnceCell::new(),

            inner_path: param.inner_path,
            referer_object,

            device_id,

            object_id: param.object_id,
            object,

            debug_info: OnceCell::new(),

            handler_req: OnceCell::new(),
        }
    }

    fn dec(mut self, dec_id: ObjectId) -> Self {
        self.dec_id_str = Self::calc_dec(Some(&dec_id));
        self
    }

    pub fn req_path(mut self, req_path: impl Into<String>) -> Self {
        self.req_path = Some(req_path.into());
        self
    }

    pub fn inner_path(mut self, inner_path: impl Into<String>) -> Self {
        self.inner_path = Some(inner_path.into());
        self
    }

    pub fn referer_object(mut self, referer_object: Vec<NDNDataRefererObject>) -> Self {
        self.referer_object = referer_object;
        self
    }

    pub fn debug_info(self, info: impl Into<String>) -> Self {
        self.debug_info.set(info.into()).unwrap();
        self
    }

    fn calc_dec(dec_id: Option<&ObjectId>) -> String {
        if let Some(id) = &dec_id {
            if *id == cyfs_core::get_system_dec_app().object_id() {
                "system".to_owned()
            } else {
                id.to_string()
            }
        } else {
            "system".to_owned()
        }
    }

    /*
    pub fn init(&mut self) -> BuckyResult<()> {

        // resource/object/location延迟初始化
        Ok(())
    }
    */

    // 初始化所有的资源路径
    async fn init_res(&self) {
        let mut list = self.create_path_res();

        if self.object_id.is_some() {
            if let Ok(path) = self.create_type_res().await {
                list.push(path);
            } else {
                // 如果失败，那么不适用type_res进行匹配
            }
        }

        debug!("init res list as: {:?}", list);
        self.req_path_list.set(list).unwrap();
    }

    fn create_path_res(&self) -> Vec<String> {
        // + 对于out request: /{target_device_id}/{dec_id}/{path}/{object_id}/{inner_path}
        // + 对于in request: /{dec_id}/{path}/{object_id}/{inner_path}
        let mut segs = vec!["".to_owned()];

        if self.action.direction == AclDirection::Out {
            segs.push(self.device_id.to_string());
        }

        segs.push(self.dec_id_str.clone());

        let mut list = vec![];

        // 不管有没有referer_object，都存在一个不使用referer-object的res-path
        let left_path = AclResource::join(&self.req_path, &self.object_id, &self.inner_path);
        segs.push(left_path);
        list.push(segs.join("/"));
        segs.pop();

        // 对于存在referer_object的，需要在路径中间放入此段
        // {req_path}/{referer_object}/{referer_inner_path}/{object_id}/{inner_path}
        if self.referer_object.len() > 0 {
            for item in &self.referer_object {
                let left_path =
                    AclResource::join(&self.req_path, &Some(item.object_id), &item.inner_path);
                let left_path =
                    AclResource::join(&Some(left_path), &self.object_id, &self.inner_path);
                segs.push(left_path);

                list.push(segs.join("/"));
                segs.pop();
            }
        }

        list
    }

    // 资源类型树
    async fn create_type_res(&self) -> BuckyResult<String> {
        let object_id = self.object_id.as_ref().unwrap();

        let category = object_id.object_category();
        let obj = if category != ObjectCategory::Standard {
            match self.get_object().await {
                Ok(obj) => Some(obj.as_ref().unwrap()),
                Err(e) => {
                    let e = BuckyError::new(e.code(), e.msg());
                    return Err(e);
                }
            }
        } else {
            None
        };

        let obj_type = if category == ObjectCategory::Standard {
            object_id.obj_type_code().to_string()
        } else {
            obj.as_ref().unwrap().obj_type().to_string()
        };

        
        let mut segs = vec!["".to_owned()];

        segs.push(category.to_string());

        // dec_app对象必须指定了dec_id
        if category == ObjectCategory::DecApp {
            match obj.as_ref().unwrap().dec_id() {
                Some(dec_id) => {
                    segs.push(dec_id.to_string());
                }
                None => {
                    let msg = format!("dec_app object without dec_id! obj={}, type={}", object_id, obj_type);
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
                }
            }
        }

        segs.push(obj_type);


        Ok(segs.join("/"))
    }

    async fn get_location(&self) -> &BuckyResult<AclGroupLocation> {
        match self.location.get() {
            Some(v) => v,
            None => {
                // 不管成功还是失败，一次acl请求里面只查询一次对象并缓存
                let mut ret = self.init_location().await;

                // 如果查询失败，比如device owner查找失败，签名校验失败, 那么强制认为是外部zone
                if ret.is_err() {
                    ret = Ok(AclGroupLocation::OuterZone);
                }
                if let Err(_) = self.location.set(ret) {
                    unreachable!("acl request location should been set only once!");
                }

                self.location.get().unwrap()
            }
        }
    }

    async fn get_object(&self) -> &BuckyResult<Option<Arc<AnyNamedObject>>> {
        match self.object.get() {
            Some(v) => v,
            None => {
                let ret = match &self.object_id {
                    Some(object_id) => {
                        // 不管成功还是失败，一次acl请求里面只查询一次对象并缓存
                        let ret = self.load_object(object_id).await.map(|o| Some(o));
                        ret
                    }
                    None => Ok(None),
                };

                if let Err(_) = self.object.set(ret) {
                    unreachable!("acl request object should been set only once!");
                }

                self.object.get().unwrap()
            }
        }
    }

    // 从noc加载对象
    async fn load_object(&self, object_id: &ObjectId) -> BuckyResult<Arc<AnyNamedObject>> {
        let data = self.match_instance.load_object(object_id).await?;
        Ok(data.object.object.unwrap())
    }

    async fn init_location(&self) -> BuckyResult<AclGroupLocation> {
        let current_zone_id = self
            .match_instance
            .zone_manager
            .get_current_zone_id()
            .await?;
        // 对于device，必须先要解析下zone,才可以判断
        let zone_id = self
            .match_instance
            .zone_manager
            .get_zone_id(&self.device_id, None)
            .await?;

        if current_zone_id == zone_id {
            Ok(AclGroupLocation::InnerZone)
        } else {
            Ok(AclGroupLocation::OuterZone)
        }
    }

    async fn init_handler_request(&self) {
        let object = match self.get_object().await {
            Ok(object) => Some(object.clone()),
            Err(_e) => {
                // 获取对象失败了如何处理? 需要继续handler，只不过由handler决定对这种情况如何处理
                None
            }
        };

        // TODO 这里是不是要传入object_raw，节省后续在handler时候的to_vec一次额外的编码操作?
        let object = match object {
            Some(object) => {
                Some(NONSlimObjectInfo::new(self.object_id.as_ref().unwrap().to_owned(), None, object))
            }
            None => None,
        };

        let req = AclHandlerRequest {
            protocol: self.protocol.clone(),
            action: self.action.clone(),

            device_id: self.device_id.clone(),
            dec_id: self.dec_id_str.clone(),

            object,

            inner_path: self.inner_path.clone(),
            req_path: self.req_path.clone(),
            referer_object: if self.referer_object.is_empty() {
                None
            } else {
                Some(self.referer_object.clone())
            },
        };

        let handler_req = RouterHandlerAclRequest {
            request: req,
            response: None,
        };

        if let Err(_) = self.handler_req.set(handler_req) {
            unreachable!();
        }
    }

    pub async fn get_handler_req(&self) -> &RouterHandlerAclRequest {
        match self.handler_req.get() {
            Some(v) => v,
            None => {
                self.init_handler_request().await;
                self.handler_req.get().unwrap()
            }
        }
    }
}

#[async_trait::async_trait]
impl AclRequest for AclRequestWrapper {
    fn protocol(&self) -> &RequestProtocol {
        &self.protocol
    }

    fn action(&self) -> &AclAction {
        &self.action
    }

    // 一个请求可以属于多个资源路径
    async fn resource(&self) -> &Vec<String> {
        match self.req_path_list.get() {
            Some(v) => v,
            None => {
                self.init_res().await;
                self.req_path_list.get().unwrap()
            }
        }
    }

    // zone内外
    async fn location(&self) -> BuckyResult<&AclGroupLocation> {
        match self.get_location().await {
            Ok(l) => Ok(l),
            Err(e) => {
                let e = BuckyError::new(e.code(), e.msg());
                Err(e)
            }
        }
    }

    // 所属dec
    fn dec(&self) -> &str {
        self.dec_id_str.as_ref()
    }

    fn device(&self) -> &DeviceId {
        &self.device_id
    }

    // 目标对象
    fn object_id(&self) -> Option<&ObjectId> {
        self.object_id.as_ref()
    }

    async fn object(&self) -> BuckyResult<Option<&Arc<AnyNamedObject>>> {
        match self.get_object().await {
            Ok(obj) => Ok(obj.as_ref()),
            Err(e) => {
                let e = BuckyError::new(e.code(), e.msg());
                Err(e)
            }
        }
    }

    // 用以调试诊断的信息
    fn debug_info(&self) -> &str {
        self.debug_info.get_or_init(|| "".to_owned()).as_str()
    }

    // 用以display
    fn display(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "protocol: {:?}", self.protocol())?;
        write!(f, ", action: {:?}", self.action())?;

        match self.location.get() {
            Some(v) => {
                write!(f, ", location: [{:?}]", v)?;
            }
            None => {}
        };

        write!(f, ", dec: {}", self.dec())?;
        write!(f, ", device: {}", self.device())?;

        if let Some(object_id) = self.object_id() {
            write!(f, ", object: {}", object_id)?;
        }
        
        match self.req_path_list.get() {
            Some(v) => {
                write!(f, ", res: [{:?}]", v)?;
            }
            None => {}
        };
        Ok(())
    }

    // 获取handler_request
    async fn handler_req(&self) -> &RouterHandlerAclRequest {
        self.get_handler_req().await
    }
}

impl std::fmt::Display for &dyn AclRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.display(f)?;
        Ok(())
    }
}
