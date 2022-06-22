//use crate::app_manager_ex::USER_APP_LIST;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use log::*;

const DEFAULT_CMD_LIST: &str = "default";
const APP_MAIN_PATH: &str = "/app";

pub struct NonHelper {
    shared_stack: Option<SharedCyfsStack>,
    owner: Option<ObjectId>,
}

impl NonHelper {
    pub fn new() -> Self {
        Self {
            shared_stack: None,
            owner: None,
        }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        let dec_id = Some(get_system_dec_app().object_id().clone());
        let stack = SharedCyfsStack::open_default(dec_id).await?;
        stack.wait_online(None).await?;
        self.owner = stack.local_device().desc().owner().clone();
        self.shared_stack = Some(stack);
        info!("owner:{:?}", self.owner);
        Ok(())
    }

    pub fn get_owner(&self) -> ObjectId {
        self.owner.unwrap().clone()
    }

    pub async fn get_local_status(&self, app_id: &DecAppId) -> BuckyResult<AppLocalStatus> {
        let status_path = format!("{}/{}/local_status", APP_MAIN_PATH, app_id.to_string());

        let v = self.load_from_map(&status_path).await?;
        let status_id = v.ok_or(BuckyError::from(BuckyErrorCode::NotFound))?;
        let resp = self.get_object(&status_id, None, 0).await?;
        AppLocalStatus::clone_from_slice(&resp.object.object_raw)
    }

    pub async fn get_app_local_list(&self) -> BuckyResult<AppLocalList> {
        let v = self.load_from_map(APP_LOCAL_LIST_PATH).await?;
        let list_id = v.ok_or(BuckyError::from(BuckyErrorCode::NotFound))?;
        let resp = self.get_object(&list_id, None, 0).await?;
        AppLocalList::clone_from_slice(&resp.object.object_raw)
    }

    pub async fn get_app_cmd_list(&self) -> BuckyResult<AppCmdList> {
        let v = self.load_from_map(CMD_LIST_PATH).await?;
        let list_id = v.ok_or(BuckyError::from(BuckyErrorCode::NotFound))?;
        let resp = self.get_object(&list_id, None, 0).await?;
        AppCmdList::clone_from_slice(&resp.object.object_raw)
    }

    pub async fn get_dec_app(&self, app_id: &ObjectId) -> BuckyResult<DecApp> {
        // DecApp会更新，这里要主动从远端获取
        let resp = self
            .get_object(app_id, None, CYFS_ROUTER_REQUEST_FLAG_FLUSH)
            .await?;
        let app = DecApp::clone_from_slice(&resp.object.object_raw)?;
        Ok(app)
    }

    async fn load_from_map(&self, path: &str) -> BuckyResult<Option<ObjectId>> {
        let op_env = self
            .shared_stack
            .as_ref()
            .unwrap()
            .root_state_stub(None)
            .create_path_op_env()
            .await?;
        op_env.get_by_path(path).await
    }

    pub async fn get_object(
        &self,
        obj_id: &ObjectId,
        target: Option<ObjectId>,
        flag: u32,
    ) -> BuckyResult<NONGetObjectOutputResponse> {
        self.shared_stack
            .as_ref()
            .unwrap()
            .non_service()
            .get_object(NONGetObjectRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: NONAPILevel::Router,
                    target,
                    flags: flag,
                },
                object_id: obj_id.clone(),
                inner_path: None,
            })
            .await
    }

    pub async fn put_object<D, T, N>(&self, obj: &N) -> BuckyResult<NONPutObjectOutputResponse>
    where
        D: ObjectType,
        T: RawEncode,
        N: RawConvertTo<T>,
        N: NamedObject<D>,
        <D as ObjectType>::ContentType: BodyContent,
    {
        self.shared_stack
            .as_ref()
            .unwrap()
            .non_service()
            .put_object(NONPutObjectRequest {
                common: NONOutputRequestCommon::new(NONAPILevel::Router),
                object: NONObjectInfo::new(obj.desc().calculate_id(), obj.to_vec()?, None),
            })
            .await
    }

    // send cmds without reponse object
    pub async fn post_object_without_resp<D, T, N>(&self, obj: &N) -> BuckyResult<()>
    where
        D: ObjectType,
        T: RawEncode,
        N: RawConvertTo<T>,
        N: NamedObject<D>,
        <D as ObjectType>::ContentType: BodyContent,
    {
        let req =
            NONPostObjectOutputRequest::new_router(None, obj.desc().calculate_id(), obj.to_vec()?);
        let ret = self
            .shared_stack
            .as_ref()
            .unwrap()
            .non_service()
            .post_object(req)
            .await;

        match ret {
            Ok(_) => Ok(()),
            Err(e) => match e.code() {
                BuckyErrorCode::Ok => Ok(()),
                _ => Err(e),
            },
        }
    }
}
