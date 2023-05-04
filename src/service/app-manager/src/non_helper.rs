//use crate::app_manager_ex::USER_APP_LIST;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use log::*;
use serde::Serialize;

const APP_MAIN_PATH: &str = "/app";

/*app related object path
//user
/system/app/{dec-id}/versions
/system/app/{dec-id}/local_status
/system/app/names/${app-name}

//manager
/system/app/manager/cmd_list
/system/app/manager/local_list
*/

pub struct NonHelper {
    owner: ObjectId,
    shared_stack: SharedCyfsStack,
}

impl NonHelper {
    pub fn new(owner: ObjectId, shared_stack: SharedCyfsStack) -> Self {
        Self {
            owner,
            shared_stack,
        }
    }

    fn get_local_status_path(app_id: &DecAppId) -> String {
        format!("{}/{}/local_status", APP_MAIN_PATH, app_id.to_string())
    }

    fn get_app_dir_path_with_ver(app_id: &DecAppId, ver: &str) -> String {
        format!("{}/{}/versions/{}", APP_MAIN_PATH, app_id.to_string(), ver)
    }

    fn get_app_dir_path_current(app_id: &DecAppId) -> String {
        format!("{}/{}/versions/current", APP_MAIN_PATH, app_id.to_string())
    }

    fn get_app_name_register_path(app_name: &str) -> String {
        format!("{}/names/{}", APP_MAIN_PATH, app_name)
    }

    pub async fn put_local_status(&self, status: &AppLocalStatus) -> BuckyResult<()> {
        let app_id = status.app_id();
        let object_id = status.desc().calculate_id();

        // 直接走non接口也可以put，只是put到自己的OOD上并签名
        match self.put_object(status).await {
            Ok(_) => {
                info!("put status to ood success! status:{}", status.output());
            }
            Err(e) => {
                error!(
                    "put status to ood failed! status:{}, err: {}",
                    status.output(),
                    e
                );
                return Err(e);
            }
        };

        let status_path = Self::get_local_status_path(app_id);

        if let Err(e) = self.store_on_map(&status_path, &object_id).await {
            error!("store status on map failed. status:{}", status.output());
            return Err(e);
        }

        // 上报到ood-daemon
        /*
        {
            "auto_update": true,
            "id": "9tGpLNnDwJ1nReZqJgWev5eoe23ygViGDC4idnCK1Dy5",
            "name": "app-manager",
            "process_state": "Run",
            "version": "1.0.0.713"
        }
         */
        #[derive(Serialize)]
        struct AppStatusInfo {
            pub auto_update: bool,
            pub id: DecAppId,
            pub name: String,
            pub process_state: String,
            pub version: String
        }
        impl From<&AppLocalStatus> for AppStatusInfo {
            fn from(value: &AppLocalStatus) -> Self {
                Self {
                    auto_update: value.auto_update(),
                    id: value.app_id().clone(),
                    name: "unknown".to_string(),
                    process_state: value.status().to_string(),
                    version: value.version().unwrap_or("unknown").to_owned(),
                }
            }
        }

        let mut info = AppStatusInfo::from(status);
        let app_id = info.id.object_id();
        if let Ok(resp) = self.shared_stack
            .non_service()
            .get_object(NONGetObjectRequest::new_noc(app_id.clone(), None))
            .await {
            if let Ok(app) = DecApp::clone_from_slice(&resp.object.object_raw) {
                info.name = app.name().to_owned();
            }
        }

        let _ = surf::post(format!("http://127.0.0.1:{}/service_status/{}", OOD_DAEMON_LOCAL_STATUS_PORT, &app_id)).body(serde_json::to_value(info).unwrap()).send().await;

        Ok(())
    }

    pub async fn get_app_setting_obj(&self, app_id: &DecAppId) -> AppSetting {
        let setting_path = format!("{}/{}", APP_SETTING_MAIN_PATH, app_id.to_string());

        match self.load_from_map(&setting_path).await {
            Ok(v) => {
                if let Some(obj_id) = v {
                    match self.get_object(&obj_id, None, 0).await {
                        Ok(resp) => match AppSetting::clone_from_slice(&resp.object.object_raw) {
                            Ok(app_setting) => {
                                return app_setting;
                            }
                            Err(e) => {
                                warn!("decode app setting failed!, appId:{}, {}", app_id, e);
                            }
                        },
                        Err(e) => {
                            warn!("get app setting failed!, appId:{}, {}", app_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("get app setting id from map failed, app:{}, {}", app_id, e);
            }
        };

        //如果没有取到，则创建，并且put
        // TODO 挂到map上？计划做到AppLocalStatus里
        let app_setting = AppSetting::create(self.owner.clone(), app_id.clone());
        let _ = self.put_object(&app_setting).await;

        app_setting
    }

    pub async fn get_local_status(&self, app_id: &DecAppId) -> AppLocalStatus {
        let status_path = Self::get_local_status_path(app_id);

        match self.load_from_map(&status_path).await {
            Ok(v) => {
                if let Some(status_id) = v {
                    match self.get_object(&status_id, None, 0).await {
                        Ok(resp) => match AppLocalStatus::clone_from_slice(&resp.object.object_raw)
                        {
                            Ok(local_status) => {
                                info!(
                                    "get local status success. app:{}, status:{}",
                                    app_id, status_id
                                );
                                return local_status;
                            }
                            Err(e) => {
                                warn!(
                                    "decode app local status failed!, app:{}, status:{}, {}",
                                    app_id, status_id, e
                                );
                            }
                        },
                        Err(e) => {
                            warn!(
                                "get app local status failed!, app:{}, status:{}, {}",
                                app_id, status_id, e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                error!(
                    "get app local status id from map failed, app:{}, {}",
                    app_id, e
                );
            }
        };

        //如果没有取到，则创建，并且put
        info!("local status obj not found, will create it, app:{}", app_id);
        let local_status = AppLocalStatus::create(self.owner.clone(), app_id.clone());
        let _ = self.put_local_status(&local_status).await;

        local_status
    }

    pub async fn get_app_local_list(&self) -> AppLocalList {
        match self.load_from_map(APP_LOCAL_LIST_PATH).await {
            Ok(v) => {
                if let Some(list_id) = v {
                    match self.get_object(&list_id, None, 0).await {
                        Ok(resp) => match AppLocalList::clone_from_slice(&resp.object.object_raw) {
                            Ok(local_list) => {
                                info!("get local list success. list:{}", list_id);
                                return local_list;
                            }
                            Err(e) => {
                                warn!("decode local list failed!, list:{}, {}", list_id, e);
                            }
                        },
                        Err(e) => {
                            warn!("get local list failed!, list:{}, {}", list_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("get local list from map failed, {}", e);
            }
        };

        //如果没有取到，则创建，并且put
        info!("will create empty local list");
        let local_list = AppLocalList::create(self.owner.clone(), APP_LOCAL_LIST_CATEGORY_APP);
        let _ = self.put_app_local_list(&local_list).await;

        local_list
    }

    pub async fn put_app_local_list(&self, list: &AppLocalList) -> BuckyResult<()> {
        let list_obj_id = list.desc().calculate_id();

        // 直接走non接口也可以put，只是put到自己的OOD上并签名
        match self.put_object(list).await {
            Ok(_) => {
                info!("put local list to ood success! list:{}", &list_obj_id);
            }
            Err(e) => {
                error!("put local list to ood failed! list:{}, {}", &list_obj_id, e);
                return Err(e);
            }
        };

        if let Err(e) = self.store_on_map(APP_LOCAL_LIST_PATH, &list_obj_id).await {
            error!("store local list on map failed. list:{}", &list_obj_id);
            return Err(e);
        }

        Ok(())
    }

    pub async fn get_cmd_list_obj(&self) -> AppCmdList {
        match self.load_from_map(CMD_LIST_PATH).await {
            Ok(v) => {
                if let Some(cmd_list_id) = v {
                    match self.get_object(&cmd_list_id, None, 0).await {
                        Ok(resp) => match AppCmdList::clone_from_slice(&resp.object.object_raw) {
                            Ok(cmd_list) => {
                                info!("get cmd list success. list:{}", cmd_list_id);
                                return cmd_list;
                            }
                            Err(e) => {
                                warn!("decode cmd list failed!, list:{}, {}", cmd_list_id, e);
                            }
                        },
                        Err(e) => {
                            warn!("get cmd list failed!, list:{}, {}", cmd_list_id, e);
                        }
                    }
                }
            }
            Err(e) => {
                error!("get cmd list from map failed, {}", e);
            }
        };

        //如果没有取到，则创建，并且put
        info!("will create empty cmd list");
        let cmd_list = AppCmdList::create(self.owner.clone(), DEFAULT_CMD_LIST);
        let _ = self.put_cmd_list(&cmd_list).await;

        cmd_list
    }

    pub async fn put_cmd_list(&self, cmd_list: &AppCmdList) -> BuckyResult<()> {
        let list_obj_id = cmd_list.desc().calculate_id();

        // 直接走non接口也可以put，只是put到自己的OOD上并签名
        match self.put_object(cmd_list).await {
            Ok(_) => {
                info!("put cmd list to ood success! list:{}", &list_obj_id);
            }
            Err(e) => {
                error!("put cmd list to ood failed! list:{}, {}", &list_obj_id, e);
                return Err(e);
            }
        };

        if let Err(e) = self.store_on_map(CMD_LIST_PATH, &list_obj_id).await {
            error!("store cmd list on map failed. list:{}", &list_obj_id);
            return Err(e);
        }

        Ok(())
    }

    pub async fn get_dec_app(
        &self,
        app_id: &ObjectId,
        app_owner: Option<ObjectId>,
    ) -> BuckyResult<DecApp> {
        // DecApp会更新，这里要主动从远端获取
        let resp = self
            .get_object(app_id, app_owner, CYFS_ROUTER_REQUEST_FLAG_FLUSH)
            .await?;
        let app = DecApp::clone_from_slice(&resp.object.object_raw)?;
        Ok(app)
    }

    pub async fn save_app_web_dir(
        &self,
        app_id: &DecAppId,
        ver: &str,
        web_dir_id: &ObjectId,
    ) -> BuckyResult<()> {
        let app_web_path_ver = Self::get_app_dir_path_with_ver(app_id, ver);
        if let Err(e) = self.store_on_map(&app_web_path_ver, web_dir_id).await {
            warn!(
                "store app web file with ver on obj map failed, web path:{}, err:{}",
                app_web_path_ver, e
            );
            return Err(e);
        }

        let app_web_path_current = Self::get_app_dir_path_current(app_id);
        if let Err(e) = self.store_on_map(&app_web_path_current, web_dir_id).await {
            warn!(
                "store app web file current on obj map failed, web path:{}, err:{}",
                app_web_path_current, e
            );
            return Err(e);
        }

        info!(
            "save app web dir success, app:{}, ver:{}, dir:{}",
            app_id, ver, web_dir_id
        );

        Ok(())
    }

    pub async fn remove_app_web_dir(
        &self,
        app_id: &DecAppId,
        ver: &str,
        web_dir_id: &ObjectId,
    ) -> BuckyResult<()> {
        let app_web_path_ver = Self::get_app_dir_path_with_ver(app_id, ver);
        if let Err(e) = self
            .delete_from_map(&app_web_path_ver, Some(web_dir_id.clone()))
            .await
        {
            warn!(
                "delete app web file with ver from obj map failed, web path:{}, err:{}",
                app_web_path_ver, e
            );
            return Err(e);
        }

        let app_web_path_current = Self::get_app_dir_path_current(app_id);
        if let Err(e) = self
            .delete_from_map(&app_web_path_current, Some(web_dir_id.clone()))
            .await
        {
            warn!(
                "delete app web file current from obj map failed, web path:{}, err:{}",
                app_web_path_current, e
            );
            return Err(e);
        }

        info!(
            "remove app web dir success, app:{}, ver:{}, dir:{}",
            app_id, ver, web_dir_id
        );

        Ok(())
    }

    pub async fn register_app_name(&self, app_name: &str, app_id: &DecAppId) -> BuckyResult<()> {
        let app_name_path = Self::get_app_name_register_path(app_name);
        if let Err(e) = self.store_on_map(&app_name_path, app_id.object_id()).await {
            warn!(
                "register app name on obj map failed, app name:{}, app id: {}, err:{}",
                app_name, app_id, e
            );
            return Err(e);
        }
        info!(
            "register app name success, app:{}, name:{}",
            app_id, app_name
        );

        Ok(())
    }

    pub async fn unregister_app_name(&self, app_name: &str, app_id: &DecAppId) -> BuckyResult<()> {
        let app_name_path = Self::get_app_name_register_path(app_name);
        if let Err(e) = self
            .delete_from_map(&app_name_path, Some(app_id.object_id().clone()))
            .await
        {
            warn!(
                "unregister app name from obj map failed, app name:{}, app id: {}, err:{}",
                app_name, app_id, e
            );
            return Err(e);
        }

        info!(
            "unregister app name success, app:{}, name:{}",
            app_id, app_name
        );

        Ok(())
    }

    pub async fn store_on_map(&self, path: &str, obj_id: &ObjectId) -> BuckyResult<()> {
        let op_env = self
            .shared_stack
            .root_state_stub(None, None)
            .create_path_op_env()
            .await?;
        op_env.lock(vec![path.to_owned()], 0).await?;
        op_env.set_with_path(path, obj_id, None, true).await?;
        op_env.commit().await?;

        Ok(())
    }

    pub async fn delete_from_map(
        &self,
        path: &str,
        prev_value: Option<ObjectId>,
    ) -> BuckyResult<()> {
        let op_env = self
            .shared_stack
            .root_state_stub(None, None)
            .create_path_op_env()
            .await?;
        op_env.lock(vec![path.to_owned()], 0).await.unwrap();
        op_env.remove_with_path(path, prev_value).await?;
        op_env.commit().await?;

        Ok(())
    }

    async fn load_from_map(&self, path: &str) -> BuckyResult<Option<ObjectId>> {
        let op_env = self
            .shared_stack
            .root_state_stub(None, None)
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
        let mut req = NONGetObjectRequest::new_router(target, obj_id.clone(), None);
        req.common.flags = flag;
        self.shared_stack
            .non_service()
            .get_object(req)
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
            .non_service()
            .put_object(NONPutObjectRequest {
                common: NONOutputRequestCommon::new(NONAPILevel::Router),
                object: NONObjectInfo::new(obj.desc().calculate_id(), obj.to_vec()?, None),
                access: None,
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
        let mut req =
            NONPostObjectOutputRequest::new_router(None, obj.desc().calculate_id(), obj.to_vec()?);
        req.common.req_path = Some(CYFS_SYSTEM_APP_VIRTUAL_PATH.to_owned());
        let ret = self.shared_stack.non_service().post_object(req).await;

        match ret {
            Ok(_) => Ok(()),
            Err(e) => match e.code() {
                BuckyErrorCode::Ok => Ok(()),
                _ => Err(e),
            },
        }
    }
}
