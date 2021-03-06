use crate::dapp::DApp;
use crate::docker_api::*;
use crate::package::AppPackage;
use cyfs_base::*;
use cyfs_client::NamedCacheClient;
use cyfs_core::*;
use log::*;
use cyfs_lib::*;
use cyfs_util::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

pub type AppActionResult<T> = std::result::Result<T, SubErrorCode>;

pub struct PermissionNode {
    key: String,
    reason: String,
}

pub struct AppController {
    shared_stack: SharedCyfsStack,
    owner: ObjectId,
    named_cache_client: NamedCacheClient,
    docker_api: Option<DockerApi>,
    use_docker: bool,
}

impl AppController {
    pub fn new(
        shared_stack: SharedCyfsStack,
        owner: ObjectId,
        named_cache_client: NamedCacheClient,
        use_docker: bool,
    ) -> Self {
        // check platform
        let docker_api;
        if use_docker {
            docker_api = Some(DockerApi::new());
        } else {
            docker_api = None;
        }

        Self {
            shared_stack,
            owner,
            named_cache_client,
            docker_api,
            use_docker,
        }
    }

    //返回isNoService，还有webDir
    pub async fn install_app(
        &self,
        app_id: &DecAppId,
        version: &str,
        dec_app: &DecApp,
    ) -> AppActionResult<(bool, Option<ObjectId>)> {
        info!("try to install app:{}, ver:{}", app_id, version);
        let source_id = dec_app.find_source(version).map_err(|e| {
            error!(
                "app {} cannot find source for ver {}, err: {}",
                app_id, version, e
            );
            SubErrorCode::DownloadFailed
        })?;
        let owner_id_str = self.get_owner_id_str(&app_id).await;
        let pkg = AppPackage::new(
            &app_id.to_string(),
            &source_id.to_string(),
            &owner_id_str,
            version,
        );
        // 返回了安装的service路径和web路径
        let (service_dir, web_dir) = pkg.install(&self.named_cache_client).await.map_err(|e| {
            error!("install app {} failed, {}", app_id, e);
            SubErrorCode::DownloadFailed
        })?;

        let web_dir_id = if web_dir.exists() {
            let pub_resp = self
                .shared_stack
                .trans()
                .publish_file(&TransPublishFileOutputRequest {
                    common: NDNOutputRequestCommon {
                        req_path: None,
                        dec_id: Some(get_system_dec_app().object_id().clone()),
                        level: Default::default(),
                        target: None,
                        referer_object: vec![],
                        flags: 0,
                    },
                    owner: self.owner,
                    local_path: web_dir,
                    chunk_size: 1024 * 1024,
                    file_id: None,
                    dirs: None,
                })
                .await
                .map_err(|e| {
                    error!(
                        "pub web dir failed when install. app {} failed, err:,{}",
                        app_id, e
                    );
                    SubErrorCode::PubDirFailed
                })?;
            info!(
                "publish web file, app:{}, fileid:{}",
                app_id, pub_resp.file_id
            );
            Some(pub_resp.file_id)
            /*let dir_id = self.shared_stack.util().build_dir_from_object_map(UtilBuildDirFromObjectMapOutputRequest {
                    common: UtilOutputRequestCommon {
                        req_path: None,
                        dec_id: Some(get_system_dec_app().object_id().clone()),
                        target: None,
                        flags: 0
                    },
                    object_map_id: pub_resp.file_id.clone(),
                    dir_type: BuildDirType::Zip
                }).await
                .and_then(|resp| {
                    info!("dir obj id: {}", resp.object_id);
                    DirId::try_from(resp.object_id)
                })
                .map_err(|e| {
                    error!(
                        "trans objmap to dir failed when install. app {}, objmap id: {}, failed, {}",
                        app_id, pub_resp.file_id, e
                    );
                    SubErrorCode::PubDirFailed
                })?;
            Some(dir_id)*/
        } else {
            None
        };
        let no_service = !service_dir.exists();

        if !no_service {
            // 获取dapp对象
            // serivce install. e.g. npm install
            let dapp = DApp::load_from_app_id(&app_id.to_string()).map_err(|e| {
                error!(
                    "get dapp instance failed when install. app {} failed, err:,{}",
                    app_id, e
                );
                SubErrorCode::LoadFailed
            })?;
            let ret = dapp.install();
            if ret.is_err() || !ret.unwrap() {
                warn!("exec install command failed. app:{}", app_id);
                return Err(SubErrorCode::CommondFailed);
            }

            //run docker install -> build image
            if self.docker_api.is_some() {
                info!("run docker install!");
                let id = app_id.to_string();

                // 可执行命令，如果有，需要再docker里 chmod +x
                let executable = {
                    let res = dapp.get_executable_binary().map_err(|e| {
                        error!(
                            "get executable failed when install. app {} failed, err:,{}",
                            app_id, e
                        );
                        SubErrorCode::LoadFailed
                    })?;
                    if res.len() == 0 {
                        None
                    } else {
                        Some(res)
                    }
                };
                let docker_api = self.docker_api.as_ref().unwrap();
                docker_api.install(&id, version, executable).await
                    .map_err(|e| {
                        error!("docker install failed. app {} failed, {}", app_id, e);
                        SubErrorCode::DockerFailed
                    })?;
            }
        }

        Ok((no_service, web_dir_id))
    }

    pub async fn uninstall_app(&self, app_id: &DecAppId) -> AppActionResult<()> {
        let _ = self.stop_app(app_id).await;
        info!("try to uninstall after stop. appid:{}", app_id);
        // 删除主机上的app目录
        let app_dir = get_app_dir(&app_id.to_string());
        if app_dir.exists() {
            std::fs::remove_dir_all(&app_dir).map_err(|e| {
                warn!("remove app dir failed, app:{}, err:{}", app_id, e);
                SubErrorCode::RemoveFailed
            })?;
        }
        let app_web_dir = get_app_web_dir(&app_id.to_string());
        if app_web_dir.exists() {
            std::fs::remove_dir_all(app_web_dir).map_err(|e| {
                warn!("remove app web dir failed, app:{}, err:{}", app_id, e);
                SubErrorCode::RemoveFailed
            })?;
        }

        // docker remove
        // 删除镜像
        if self.docker_api.is_some() {
            info!("docker instance try to uninstall app {}", app_id);
            let id = app_id.to_string();
            let docker_api = self.docker_api.as_ref().unwrap();
            // docker_api.volume_remove(&id).await; // 这里不用删除 volume 保留用户数据。
            let _ = docker_api.uninstall(&id).await.map_err(|e| {
                warn!(
                    "remove docker container and build dir failed, app:{}, err:{}",
                    app_id, e
                );
                SubErrorCode::DockerFailed
            });
        }
        Ok(())
    }

    pub async fn start_app(&self, app_id: &DecAppId, config: RunConfig) -> AppActionResult<()> {
        info!("try to start app:{}", app_id);
        let id = app_id.to_string();
        let mut dapp = DApp::load_from_app_id(&id).map_err(|e| {
            warn!("load app failed, appId: {}, err:{}", id, e);
            SubErrorCode::LoadFailed
        })?;

        if self.docker_api.is_some() {
            let cmd = dapp.get_start_cmd().unwrap();
            let cmd_param = Some(vec![cmd.to_string()]);
            info!("service cmd: {}", cmd);
            self.docker_api
                .as_ref()
                .unwrap()
                .start(&id, config, cmd_param)
                .await
                .map_err(|e| {
                    warn!("docker start failed, appId: {}, {}", app_id, e);
                    SubErrorCode::DockerFailed
                })?;
        } else {
            // 应用在主机直接运行
            info!("run app simple:{}", app_id);
            dapp.start().map_err(|e| {
                warn!("start app directly failed, appId: {}, {}", app_id, e);
                SubErrorCode::None
            })?;
        }
        Ok(())
    }

    pub async fn stop_app(&self, app_id: &DecAppId) -> AppActionResult<()> {
        let id = app_id.to_string();
        if self.docker_api.is_some() {
            match self.docker_api.as_ref().unwrap().stop(&id).await {
                Ok(_) => {
                    info!("stop docker container success!, app:{}", id);
                }
                Err(e) => {
                    warn!("stop docker failed, app:{}, err:{}", app_id, e);
                    return Err(SubErrorCode::DockerFailed);
                }
            }
        } else {
            let mut dapp = DApp::load_from_app_id(&id).map_err(|e| {
                warn!("load app failed, app:{}, err:{}", app_id, e);
                SubErrorCode::LoadFailed
            })?;
            let result = dapp.stop().map_err(|e| {
                warn!("stop app directly failed, app:{}, err:{}", app_id, e);
                SubErrorCode::Unknown
            })?;
            info!("stop dapp instance:{}", result);
        }

        Ok(())
    }

    pub async fn is_app_running(&self, app_id: &DecAppId) -> BuckyResult<bool> {
        let id = app_id.to_string();

        if self.docker_api.is_some() {
            let result = self.docker_api.as_ref().unwrap().is_running(&id).await?;
            return Ok(result);
        } else {
            let mut dapp = DApp::load_from_app_id(&id)?;
            let result = dapp.status()?;
            return Ok(result);
        }
    }

    pub async fn query_app_permission(
        &self,
        app_id: &DecAppId,
        version: &str,
        dec_app: &DecApp,
    ) -> BuckyResult<Option<HashMap<String, String>>> {
        debug!("query app permission, {}-{}", app_id, version);
        let source_id = dec_app.find_source(version).map_err(|e| {
            error!("app {} cannot find source for ver {}", app_id, version);
            e
        })?;
        let owner_id_str = self.get_owner_id_str(&app_id).await;
        let pkg = AppPackage::new(
            &app_id.to_string(),
            &source_id.to_string(),
            &owner_id_str,
            version,
        );

        let acl_dir;

        match pkg
            .download_permission_config(&self.named_cache_client)
            .await
        {
            Ok(dir) => acl_dir = dir,
            Err(e) => {
                //下载acl失败，默认没有任何权限
                warn!("download acl config failed. app: {}， err: {}", app_id, e);
                return Ok(None);
            }
        }

        let acl_file = acl_dir.join("acl.cfg");
        if !acl_file.exists() {
            info!("acl config not found. app: {}", app_id);
            return Ok(None);
        }
        let acl = File::open(acl_file)?;
        let acl_info: Value = serde_json::from_reader(acl)?;
        let acl_map = acl_info.as_object().ok_or_else(|| {
            let msg = format!("invalid acl file format: {}", acl_info);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
        info!("get acl for app:{}, acl:{:?}", app_id, acl_map);
        if acl_map.is_empty() {
            return Ok(None);
        }
        let mut permissions = HashMap::new();
        for (k, v) in acl_map {
            permissions.insert(k.to_string(), v.to_string());
        }

        Ok(Some(permissions))
    }

    // 查询app对stack的版本依赖，返回（minVer，maxVer）
    pub async fn query_app_version_dep(
        &self,
        app_id: &DecAppId,
        version: &str,
        dec_app: &DecApp,
    ) -> BuckyResult<(String, String)> {
        debug!("query app stack dep, {}-{}", app_id, version);
        let dep_dir = get_app_dep_dir(&app_id.to_string(), version);
        let dep_file = dep_dir.join("dependent.cfg");
        if dep_file.exists() {
            info!(
                "dep config already exists. app: {}, ver:{}",
                app_id, version
            );
            return self.parse_dep_config(app_id, dep_file);
        }

        let source_id = dec_app.find_source(version).map_err(|e| {
            error!("app {} cannot find source for ver {}", app_id, version);
            e
        })?;
        let owner_id_str = self.get_owner_id_str(&app_id).await;
        let pkg = AppPackage::new(
            &app_id.to_string(),
            &source_id.to_string(),
            &owner_id_str,
            version,
        );

        let _ = pkg
            .download_dep_config(dep_dir, &self.named_cache_client)
            .await
            .map_err(|e| {
                error!("download app dep {} failed, {}", app_id, e);
                e
            })?;

        self.parse_dep_config(app_id, dep_file)
    }

    fn parse_dep_config(
        &self,
        app_id: &DecAppId,
        dep_file: PathBuf,
    ) -> BuckyResult<(String, String)> {
        let default_ret = ("*".to_string(), "*".to_string());

        if !dep_file.exists() {
            //没有设置兼容性的话，默认全匹配
            info!("dep config not found. app: {}", app_id);
            return Ok(default_ret);
        }

        let dep_file = File::open(dep_file)?;
        let dep_info: Value = serde_json::from_reader(dep_file)?;
        let dep_map = dep_info.as_object().ok_or_else(|| {
            let msg = format!("invalid acl file format: {}", dep_info);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
        info!("get dep for app:{}, {:?}", app_id, dep_map);
        if dep_map.is_empty() {
            return Ok(default_ret);
        }
        let min_ver = dep_map
            .get("min")
            .unwrap_or(&serde_json::json!("*"))
            .clone();
        let max_ver = dep_map
            .get("max")
            .unwrap_or(&serde_json::json!("*"))
            .clone();

        Ok((min_ver.to_string(), max_ver.to_string()))
    }

    async fn get_owner_id_str(&self, app_id: &DecAppId) -> String {
        let mut owner_id_str: std::string::String = "".to_owned();
        let owner = self.get_owner_id(&app_id).await;
        if owner.is_some() {
            owner_id_str = owner.unwrap().to_string();
        }
        owner_id_str
    }

    async fn get_owner_id(&self, app_id: &DecAppId) -> Option<ObjectId> {
        // DecApp会更新，这里要主动从远端获取
        let resp = self
            .shared_stack
            .non_service()
            .get_object(NONGetObjectRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: NONAPILevel::Router,
                    target: None,
                    flags: CYFS_ROUTER_REQUEST_FLAG_FLUSH,
                },
                object_id: app_id.clone().into(),
                inner_path: None,
            })
            .await
            .unwrap();
        let dec_app = DecApp::clone_from_slice(&resp.object.object_raw).unwrap();

        let owner = dec_app.desc().owner().unwrap();
        info!("dec app owner {}", owner);
        match owner.obj_type_code() {
            ObjectTypeCode::Device => Some(owner),
            ObjectTypeCode::People => {
                match self
                    .shared_stack
                    .util_service()
                    .resolve_ood(UtilResolveOODOutputRequest::new(
                        app_id.object_id().to_owned(),
                        Some(owner),
                    ))
                    .await
                {
                    Ok(resp) => {
                        let ood_list = resp.device_list;
                        if !ood_list.is_empty() {
                            Some(ood_list[0].object_id().to_owned())
                        } else {
                            None
                        }
                    }
                    Err(e) => {
                        error!("get ood id fail {}", e);
                        None
                    }
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    //use cyfs_core::;
    use std::convert::TryFrom;
    use std::str::FromStr;

    async fn get_stack() -> SharedCyfsStack {
        let cyfs_stack = SharedCyfsStack::open_default(None).await.unwrap();
        cyfs_stack.wait_online(None).await;

        cyfs_stack
    }

    async fn get_app_controller() -> AppController {
        let stack = get_stack().await;
        let named_cache_client = NamedCacheClient::new();
        let device = stack.local_device();
        let owner = device
            .desc()
            .owner()
            .to_owned()
            .unwrap_or_else(|| device.desc().calculate_id());
        let app_controller = AppController::new(stack, owner, named_cache_client, false);
        app_controller
    }

    // 安装app
    #[async_std::test]
    async fn test_app_install() {
        let owner = ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap();
        let appid = DecAppId::from_str("9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2").unwrap();
        let appcmd = AppCmd::install(owner, appid, &"1.0.7".to_string(), false);

        let stack = get_stack().await;
        let result = stack
            .non_service()
            .put_object(NONPutObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: NONAPILevel::NOC,
                    target: None,
                    flags: 0,
                },
                object: NONObjectInfo {
                    object_id: appcmd.desc().calculate_id(),
                    object_raw: appcmd.to_vec().unwrap(),
                    object: None,
                },
            })
            .await
            .unwrap();
        //println!("put app cmd result {:?}", result);
        println!("put app cmd");
    }

    // 运行app
    #[async_std::test]
    async fn test_app_controller_run() {
        let app_controller = get_app_controller().await;
        let appid = DecAppId::from_str("9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2").unwrap();

        let resp = app_controller
            .start_app(
                &appid,
                RunConfig {
                    ..Default::default()
                },
            )
            .await;

        println!("resp {:?}", resp);
        let app_running = app_controller.is_app_running(&appid).await.unwrap();
        assert!(app_running);
    }

    #[async_std::test]
    async fn test_app_controller_stop() {
        let app_controller = get_app_controller().await;
        let appid = DecAppId::from_str("9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2").unwrap();
        let resp = app_controller.stop_app(&appid).await;

        println!("resp {:?}", resp);

        let app_running = app_controller.is_app_running(&appid).await.unwrap();
        assert!(!app_running);
    }

    #[async_std::test]
    async fn test_app_controller_uninstall() {
        let app_controller = get_app_controller().await;
        let appid = DecAppId::from_str("9tGpLNnYywrCAWoCcyhAcLZtrQpDZtRAg3ai2w47aap2").unwrap();
        let resp = app_controller.uninstall_app(&appid).await;
        println!("resp {:?}", resp);
    }
}
