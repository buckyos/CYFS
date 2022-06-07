use super::DeviceConfigRepo;
use crate::config::*;
use base::LOCAL_DEVICE_MANAGER;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_meta_lib::{MetaClient, MetaClientHelper, MetaMinerTarget};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
struct DeviceConfigGenerator {
    service: Vec<ServiceConfig>,
}

impl DeviceConfigGenerator {
    pub fn new() -> Self {
        Self {
            service: Vec::new(),
        }
    }

    // 尝试从本地已经保存的配置里面加载
    pub fn load_from_local(&mut self) {
        let device_config = DeviceConfig::new();
        let ret = device_config.load_config();
        if ret.is_err() {
            return;
        }

        self.service = ret.unwrap();
    }

    pub fn to_string(&mut self) -> String {
        toml::to_string(&self).unwrap()
    }

    // 同步列表
    pub fn sync_list(&mut self,  service_list: &AppList) {
        // 相同名字去重
        self.service.dedup_by(|a, b| a.name == b.name);

        self.service.retain(|item| {
            if item.id.is_empty() {
                error!("invalid service item! id is empty! name={}", item.name);
                return false;
            }

            let ret = DecAppId::from_str(&item.id);
            if ret.is_err() {
                error!("invalid service id! id={}, name={}, {}", item.id, item.name, ret.unwrap_err());
                return false;
            }

            let service_id = ret.unwrap();

            if service_list.app_list().contains_key(&service_id) {
                true
            } else {
                warn!("service removed from service list! id={}, name={}", item.id, item.name);
                false
            }
        });
    }

    pub fn update_service(&mut self, service: &DecApp, fid: &str) {
        let id = service.desc().calculate_id().to_string();
        let name = service.name();
        info!("will update service: id={}, name={}, fid={}", id, name, fid);

        for item in self.service.iter_mut() {
            if item.name == name {
                item.id = id.clone();
                item.fid = fid.to_owned();
            } else if item.id == id {
                item.name = name.to_owned();
                item.fid = fid.to_owned();
            }
        }
    }

    pub fn update_service_status(&mut self, status: &AppStatus) -> bool {
        let id = status.app_id().to_string();
        let version = status.version();
        let target_state = match status.status() {
            true => ServiceState::RUN,
            false => ServiceState::STOP,
        };

        for item in self.service.iter_mut() {
            if item.id == id {
                let mut version_changed = false;

                // 检查状态是不是发生改变
                if item.target_state != target_state {
                    item.target_state = target_state;
                    info!(
                        "service target state changed! id={}, name={} target state: {} -> {}",
                        item.id, item.name, item.target_state, target_state
                    );
                }

                // 检查版本是不是发生改变
                if item.version != version {
                    info!(
                        "service version changed! id={}, name={} version: {} -> {}",
                        item.id, item.name, item.version, version
                    );
                    item.version = version.to_owned();

                    // 版本更新后，需要拉取最新的fid
                    version_changed = true;
                }
                return version_changed;
            }
        }

        info!("new service item: id={}, version={}", id, version);
        let mut service = ServiceConfig::new();
        service.id = id;
        service.version = version.to_owned();
        service.enable = true;
        service.target_state = target_state;
        self.service.push(service);

        // fid+name需要从DecApp对象获取

        true
    }
}

pub struct DeviceConfigMetaRepo {
    desc: String,

    device_id: Option<DeviceId>,

    service_list_id: Option<ObjectId>,

    meta_client: MetaClient,

    device_config: Mutex<DeviceConfigGenerator>,
}

impl DeviceConfigMetaRepo {
    pub fn new() -> Self {
        let meta_client = MetaClient::new_target(MetaMinerTarget::default());

        Self {
            desc: String::from(""),
            device_id: None,
            service_list_id: None,
            meta_client,
            device_config: Mutex::new(DeviceConfigGenerator::new()),
        }
    }

    pub fn init(
        &mut self,
        config_desc: &str,
        version: &ServiceListVersion,
    ) -> Result<(), BuckyError> {
        assert!(self.desc.len() == 0);
        self.desc = config_desc.to_owned();

        // 首先加载device，用作ServiceList的owner
        let device_id = self.load_device()?;
        let version = version.to_string();

        // 计算ServiceList对象id
        let service_list_id = AppList::generate_id(device_id.object_id().to_owned(), &version, APPLIST_SERVICE_CATEGORY);

        info!("device config repo: device_id={}, app_list_id={}, version={}", device_id, service_list_id, version);

        self.service_list_id = Some(service_list_id);
        self.device_id = Some(device_id);

        // 加载本地缓存
        self.device_config.lock().unwrap().load_from_local();

        Ok(())
    }

    fn load_device(&self) -> BuckyResult<DeviceId> {
        assert!(self.device_id.is_none());

        let ret = LOCAL_DEVICE_MANAGER.load(&self.desc);
        if let Err(e) = ret {
            error!("load config desc failed! desc={}, err={}", self.desc, e);
            return Err(e);
        }

        let device = ret.unwrap();
        let ret = device.device.desc().device_id();

        Ok(ret)
    }

    async fn load_service_list(&self) -> BuckyResult<AppList> {
        assert!(self.service_list_id.is_some());

        let object_id = self.service_list_id.as_ref().unwrap();
        let ret = MetaClientHelper::get_object(&self.meta_client, object_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load service list object from meta chain but not found! id={}",
                object_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (_, object_raw) = ret.unwrap();

        // 解码
        let list = AppList::clone_from_slice(&object_raw).map_err(|e| {
            let msg = format!("decode service list object failed! id={}, {}", object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        info!("load service list object success! id={:?}, list={:?}", self.service_list_id, list.app_list());

        Ok(list)
    }

    async fn load_service(&self, service_id: &ObjectId) -> BuckyResult<DecApp> {
        let ret = MetaClientHelper::get_object(&self.meta_client, service_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load service object from meta chain but not found! id={}",
                service_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (_, object_raw) = ret.unwrap();

        // 解码
        let service = DecApp::clone_from_slice(&object_raw).map_err(|e| {
            let msg = format!("decode service object failed! id={}, {}", service_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(service)
    }

    async fn load_service_dir(&self, dir_id: &ObjectId) -> BuckyResult<Dir> {
        let ret = MetaClientHelper::get_object(&self.meta_client, dir_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load service dir from meta chain but not found! id={}",
                dir_id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (_, object_raw) = ret.unwrap();

        // 解码
        let dir = Dir::clone_from_slice(&object_raw).map_err(|e| {
            let msg = format!("decode service dir failed! id={}, {}", dir_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(dir)
    }

    // 从dir里面加载当前target对应的fid
    fn load_fid(&self, dir_id: &str, dir: Dir) -> BuckyResult<String> {
        let mut target = get_system_config().target.clone();

        match dir.desc().content().obj_list() {
            NDNObjectInfo::ObjList(entries) => {
                let ret = entries.object_map().get(&target);
                if ret.is_none() {

                    // 添加zip后缀，再次判断
                    target = format!("{}.zip", target);
                    let ret = entries.object_map().get(&target);
                    if ret.is_none() {
                        let msg = format!(
                            "target fid not found in service dir! dir={}, target={}",
                            dir_id, target
                        );
                        error!("{}", msg);
    
                        return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }
                }

                let fid = format!("{}/{}", dir_id, target);

                info!(
                    "get fid from dir, dir={}, fid={}",
                    dir_id, fid
                );

                Ok(fid)
            }
            NDNObjectInfo::Chunk(_chunk_id) => {
                let msg = format!("chunk mode not support! dir={}", dir_id);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    async fn load_service_fid(&self, service_id: &ObjectId, version: &str) -> BuckyResult<()> {
        let service = self.load_service(service_id).await?;

        let ret = service.find_source(version);
        if ret.is_err() {
            let msg = format!(
                "get version from service object failed! id={}, version={}",
                service_id, version
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // 加载dir对象
        let dir_id = ret.unwrap();
        let dir = self.load_service_dir(&dir_id).await?;

        // 查找当前平台对应的fid
        let dir_id = dir_id.to_string();
        let fid = self.load_fid(&dir_id, dir)?;

        // 更新
        self.device_config
            .lock()
            .unwrap()
            .update_service(&service, &fid);

        Ok(())
    }

    async fn update_service_list_to_device_config(&self, service_list: &AppList) {
        for (id, status) in service_list.app_list() {
            debug!("got service item from service list: {}", id);

            let version_changed = self
                .device_config
                .lock()
                .unwrap()
                .update_service_status(status);
            if version_changed {
                let version = status.version();

                if let Err(e) = self.load_service_fid(id.object_id(), version).await {
                    error!(
                        "load service fid failed! id={}, version={}, {}",
                        id, version, e
                    );
                }
            }
        }

        // info!("list {:?}", self.device_config.lock().uwnrap().service);

        // 移除已经不存在的service
        self.device_config.lock().unwrap().sync_list(service_list);
    }
}

#[async_trait]
impl DeviceConfigRepo for DeviceConfigMetaRepo {
    fn get_type(&self) -> &'static str {
        "meta"
    }

    async fn fetch(&self) -> BuckyResult<String> {
        // 从mete-chain拉取对应的service_list
        let service_list = self.load_service_list().await?;

        self.update_service_list_to_device_config(&service_list)
            .await;

        let device_config_str = self.device_config.lock().unwrap().to_string();

        info!(
            "load device_config from meta: device_id={}, config={}",
            self.device_id.as_ref().unwrap(),
            device_config_str
        );

        Ok(device_config_str)
    }
}
