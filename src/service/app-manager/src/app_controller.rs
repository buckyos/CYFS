use crate::app_acl_util::*;
use crate::dapp::DApp;
use crate::docker_api::*;
use crate::package::AppPackage;
use cyfs_base::*;
use cyfs_client::{NamedCacheClient, NamedCacheClientConfig};
use cyfs_core::{DecApp, DecAppId, DecAppObj, SubErrorCode};
use cyfs_lib::*;
use cyfs_util::*;
use log::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use async_std::prelude::StreamExt;
use once_cell::sync::OnceCell;
use app_manager_lib::AppManagerConfig;

pub type AppActionResult<T> = Result<T, SubErrorCode>;

pub struct PermissionNode {
    key: String,
    reason: String,
}

pub struct AppController {
    shared_stack: OnceCell<SharedCyfsStack>,
    owner: ObjectId,
    docker_api: DockerApi,
    named_cache_client: OnceCell<NamedCacheClient>,
    sn_hash: RwLock<HashValue>,
    config: AppManagerConfig,
    dapp_instance: RwLock<HashMap<DecAppId, DApp>>,
}

async fn get_sn_list(stack: &SharedCyfsStack) -> BuckyResult<Vec<Device>> {
    stack.wait_online(Some(Duration::from_secs(5))).await?;

    let info = stack.util().get_device_static_info(UtilGetDeviceStaticInfoOutputRequest::new()).await?;
    let mut devices = vec![];
    for sn_id in &info.info.known_sn_list {
        let resp = stack.non_service().get_object(NONGetObjectOutputRequest::new_noc(sn_id.object_id().clone(), None)).await?;
        devices.push(Device::clone_from_slice(&resp.object.object_raw)?);
    }

    Ok(devices)
}

impl AppController {
    pub fn new(config: AppManagerConfig, owner: ObjectId) -> Self {
        Self {
            shared_stack: OnceCell::new(),
            owner,
            named_cache_client: OnceCell::new(),
            sn_hash: RwLock::new(HashValue::default()),
            docker_api: DockerApi::new(),
            config,
            dapp_instance: RwLock::new(HashMap::new())
        }
    }

    pub async fn prepare_start(
        &self, shared_stack: SharedCyfsStack,
    ) -> BuckyResult<()> {
        let sn_list = get_sn_list(&shared_stack).await.unwrap_or_else(|e| {
            error!("get sn list from stack err {}, use built-in sn list", e);
            get_builtin_sn_desc().as_slice().iter().map(|(_, device)| device.clone()).collect()
        });

        let area = shared_stack.local_device_id().object_id().info().into_area();
        info!("get area from stack: {:?}", area);

        let sn_hash = hash_data(&sn_list.to_vec().unwrap());
        *self.sn_hash.write().unwrap() = sn_hash;
        self.shared_stack.set(shared_stack).map_err(|_|{
            BuckyError::from(BuckyErrorCode::AlreadyExists)
        })?;

        let mut config = NamedCacheClientConfig::default();
        config.sn_list = Some(sn_list);
        config.area = area;
        config.conn_strategy = cyfs_client::ConnStrategy::TcpFirst;
        config.timeout = Duration::from_secs(10*60);
        config.tcp_file_manager_port = 5312;
        config.tcp_chunk_manager_port = 5310;
        let mut named_cache_client = NamedCacheClient::new(config);
        named_cache_client.init().await?;
        self.named_cache_client.set(named_cache_client).map_err(|_|{
            BuckyError::from(BuckyErrorCode::AlreadyExists)
        })?;
        Ok(())
    }

    pub async fn start_monitor_sn(this: Arc<AppController>) {
        // 起一个5分钟的timer，查sn
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(5*60));
            while let Some(_) = interval.next().await {
                match get_sn_list(this.shared_stack.get().unwrap()).await {
                    Ok(sn_list) => {
                        let sn_hash = hash_data(&sn_list.to_vec().unwrap());
                        let old_hash = this.sn_hash.read().unwrap().clone();
                        if old_hash != sn_hash {
                            info!("sn list from stack changed, {:?}", &sn_list);
                            match this.named_cache_client.get().unwrap().reset_known_sn_list(sn_list) {
                                Ok(_) => {
                                    *this.sn_hash.write().unwrap() = sn_hash;
                                }
                                Err(e) => {
                                    error!("change named cache client sn list err {}", e);
                                }
                            }

                        }
                    }
                    Err(e) => {
                        error!("get sn list from stack err {}, skip", e);
                        continue
                    }
                }

            }
        });
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
                "app:{} cannot find source for ver {}, err: {}",
                app_id, version, e
            );
            SubErrorCode::DownloadFailed
        })?;
        let owner_id = self.get_owner_id(&app_id).await.map_err(|_e| {
            error!("get app {} owner id failed", &app_id);
            SubErrorCode::LoadFailed
        })?;
        let web_dir_id = AppPackage::install(&app_id, version,
                                             &source_id, &owner_id,
                                             self.named_cache_client.get().unwrap(),
                                             self.config.config.repo_mode.clone(),
                                             self.shared_stack.get().unwrap().clone())
            .await
            .map_err(|e| {
                error!("install app:{} failed, {}", app_id, e);
                SubErrorCode::DownloadFailed
            })?;
        let service_dir = get_app_dir(&app_id.to_string());

        let no_service = !service_dir.exists();

        if !no_service {
            // 获取dapp对象
            // serivce install. e.g. npm install
            let dapp = DApp::load_from_app_id(&app_id.to_string()).map_err(|e| {
                error!(
                    "get dapp instance failed when install. app:{} failed, err:,{}",
                    app_id, e
                );
                SubErrorCode::LoadFailed
            })?;

            //run docker install -> build image
            let use_docker = self.config.app_use_docker(app_id);
            info!("app {} use docker install: {}", app_id, use_docker);
            if use_docker {
                info!("run docker install!");
                let id = app_id.to_string();
                let install_cmds = dapp.get_install_cmd();
                self.docker_api
                    .install(&id, version, install_cmds)
                    .await
                    .map_err(|e| {
                        error!("docker install failed. app:{} failed, {}", app_id, e);
                        SubErrorCode::DockerFailed
                    })?;
            } else {
                let ret = dapp.install();
                if ret.is_err() || !ret.unwrap() {
                    warn!("exec install command failed. app:{}", app_id);
                    return Err(SubErrorCode::CommondFailed);
                }
            }
        }

        Ok((no_service, web_dir_id))
    }

    pub async fn uninstall_app(&self, app_id: &DecAppId, ver: &str) -> AppActionResult<()> {
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

        let app_web_dir = get_app_web_dir2(&app_id.to_string(), ver);
        if app_web_dir.exists() {
            std::fs::remove_dir_all(app_web_dir).map_err(|e| {
                warn!("remove app web dir2 failed, app:{}, err:{}", app_id, e);
                SubErrorCode::RemoveFailed
            })?;
        }

        // docker remove
        // 删除镜像
        let use_docker = self.config.app_use_docker(app_id);
        info!("app {} use docker uninstall: {}", app_id, use_docker);
        if use_docker {
            info!("docker instance try to uninstall app:{}", app_id);
            let id = app_id.to_string();
            // self.docker_api.volume_remove(&id).await; // 这里不用删除 volume 保留用户数据。
            let _ = self.docker_api.uninstall(&id).await.map_err(|e| {
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

        let use_docker = self.config.app_use_docker(app_id);
        info!("app {} use docker start: {}", app_id, use_docker);
        if use_docker {
            let dapp = DApp::load_from_app_id(&id).map_err(|e| {
                warn!("load app failed, appId: {}, err:{}", id, e);
                SubErrorCode::LoadFailed
            })?;
            let cmd = dapp.get_start_cmd();
            info!("service cmd: {}", &cmd);
            self.docker_api
                .start(&id, config, cmd)
                .await
                .map_err(|e| {
                    warn!("docker start failed, appId: {}, {}", app_id, e);
                    SubErrorCode::DockerFailed
                })?;
        } else {
            // 应用在主机直接运行
            info!("run app simple:{}", app_id);
            let dapp = DApp::load_from_app_id(&app_id.to_string()).map_err(|e| {
                warn!("load app failed, appId: {}, err:{}", app_id, e);
                SubErrorCode::LoadFailed
            })?;

            dapp.start().map_err(|e| {
                warn!("start app directly failed, appId: {}, {}", app_id, e);
                SubErrorCode::CommondFailed
            })?;

            self.dapp_instance.write().unwrap().insert(app_id.clone(), dapp);
        }
        Ok(())
    }

    pub async fn stop_app(&self, app_id: &DecAppId) -> AppActionResult<()> {
        let id = app_id.to_string();
        let use_docker = self.config.app_use_docker(app_id);
        info!("app {} use docker stop: {}", app_id, use_docker);
        if use_docker {
            match self.docker_api.stop(&id) {
                Ok(_) => {
                    info!("stop docker container success!, app:{}", id);
                }
                Err(e) => {
                    warn!("stop docker failed, app:{}, err:{}", app_id, e);
                    return Err(SubErrorCode::DockerFailed);
                }
            }
        } else {
            let mut app_list = self.dapp_instance.write().unwrap();
            if let Some(dapp) = app_list.remove(app_id) {
                let result = dapp.stop().map_err(|e| {
                    warn!("stop app directly failed, app:{}, err:{}", app_id, e);
                    SubErrorCode::CommondFailed
                })?;
                info!("stop dapp instance:{}", result);
            } else {
                let dapp = DApp::load_from_app_id(&app_id.to_string()).map_err(|e| {
                    warn!("load app failed, appId: {}, err:{}", app_id, e);
                    SubErrorCode::LoadFailed
                })?;

                let result = dapp.stop().map_err(|e| {
                    warn!("stop app directly failed, app:{}, err:{}", app_id, e);
                    SubErrorCode::CommondFailed
                })?;
                info!("stop dapp instance:{}", result);
            };
        }

        Ok(())
    }

    pub async fn is_app_running(&self, app_id: &DecAppId) -> BuckyResult<bool> {
        let id = app_id.to_string();

        let use_docker = self.config.app_use_docker(app_id);
        info!("app {} use docker status: {}", app_id, use_docker);
        if use_docker {
            self.docker_api.is_running(&id)
        } else {
            if let Some(dapp) = self.dapp_instance.read().unwrap().get(app_id) {
                dapp.status()
            } else {
                let dapp = DApp::load_from_app_id(&app_id.to_string())?;
                dapp.status()
            }
        }
    }

    pub async fn get_app_permission(
        &self,
        app_id: &DecAppId,
    ) -> BuckyResult<Option<HashMap<String, String>>> {
        let acl_file = get_app_acl_dir(&app_id.to_string()).join("acl.cfg");

        if !acl_file.exists() {
            info!("acl config not found. app:{}", app_id);
            return Ok(None);
        }

        let acl_config = AppAclUtil::load_from_file(app_id, &acl_file)?;

        let _ =
            AppAclUtil::apply_acl(app_id, self.shared_stack.get().unwrap(), acl_config).await;

        //TODO: Requires users to agree to permissions, not automatic settings
        Ok(None)

        /*let acl = File::open(acl_file)?;
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

        Ok(Some(permissions))*/
    }

    // 查询app对stack的版本依赖，返回（minVer，maxVer）
    pub async fn get_app_version_dep(
        &self,
        app_id: &DecAppId,
    ) -> BuckyResult<(String, String)> {
        let dep_dir = get_app_dep_dir(&app_id.to_string());
        let dep_file = dep_dir.join("dependent.cfg");
        if dep_file.exists() {
            info!("dep config already exists. app:{}", app_id);
            return self.parse_dep_config(app_id, dep_file);
        }

        return Ok(("*".to_string(), "*.".to_string()))
    }

    fn parse_dep_config(
        &self,
        app_id: &DecAppId,
        dep_file: PathBuf,
    ) -> BuckyResult<(String, String)> {
        let default_ret = ("*".to_string(), "*".to_string());

        if !dep_file.exists() {
            //没有设置兼容性的话，默认全匹配
            info!("dep config not found. app:{}", app_id);
            return Ok(default_ret);
        }

        let dep_file = File::open(dep_file)?;
        let dep_info: Value = serde_json::from_reader(dep_file)?;
        let dep_map = dep_info.as_object().ok_or_else(|| {
            let msg = format!("invalid dep file format: {}", dep_info);
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
        let mut owner_id_str = "".to_owned();
        let owner = self.get_owner_id(&app_id).await;
        if let Ok(owner) = owner {
            owner_id_str = owner.to_string();
        }
        owner_id_str
    }

    // valid dec app must have a owner
    async fn get_owner_id(&self, app_id: &DecAppId) -> BuckyResult<ObjectId> {
        // DecApp会更新，这里要主动从远端获取
        let resp = self
            .shared_stack
            .get()
            .unwrap()
            .non_service()
            .get_object(NONGetObjectRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    source: None,
                    dec_id: None,
                    level: NONAPILevel::Router,
                    target: None,
                    flags: CYFS_ROUTER_REQUEST_FLAG_FLUSH,
                },
                object_id: app_id.clone().into(),
                inner_path: None,
            })
            .await?;
        let dec_app = DecApp::clone_from_slice(&resp.object.object_raw)?;

        let owner = dec_app.desc().owner().unwrap();
        info!("dec app owner {}", owner);
        Ok(owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    //use cyfs_core::;
    use cyfs_core::{AppCmd, AppCmdObj};
    use std::convert::TryFrom;
    use std::str::FromStr;

    async fn get_stack() -> SharedCyfsStack {
        let cyfs_stack = SharedCyfsStack::open_default(None).await.unwrap();
        cyfs_stack.wait_online(None).await;

        cyfs_stack
    }

    async fn get_app_controller() -> AppController {
        let stack = get_stack().await;
        let named_cache_client = NamedCacheClient::new(NamedCacheClientConfig::default());
        let device = stack.local_device();
        let owner = device
            .desc()
            .owner()
            .to_owned()
            .unwrap_or_else(|| device.desc().calculate_id());

        let mut app_controller = AppController::new(AppManagerConfig::default(), stack, owner);
        app_controller.prepare_start().await;

        app_controller
        //let app_controller = AppController::new(stack, owner, named_cache_client, false);
        //app_controller
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
                    source: None,
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
                access: None,
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
