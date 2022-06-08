use crate::zone::ZoneManager;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_debug::Mutex;
use cyfs_lib::*;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppService {
    device_id: DeviceId,
    noc: Arc<Box<dyn NamedObjectCache>>,
    cache: Arc<Mutex<HashMap<String, DecAppId>>>,
    zone_manager: ZoneManager,
}

impl AppService {
    pub fn new(
        device_id: DeviceId,
        noc: Box<dyn NamedObjectCache>,
        zone_manager: ZoneManager,
    ) -> Self {
        Self {
            device_id,
            noc: Arc::new(noc),
            cache: Arc::new(Mutex::new(HashMap::new())),
            zone_manager,
        }
    }

    pub async fn get_app_web_dir(&self, name: &str) -> BuckyResult<Option<ObjectId>> {
        let status = self.get_app_local_status(name).await?;
        if status.is_none() {
            return Ok(None);
        }

        let status = status.unwrap();
        info!(
            "app status: name={}, dec={}, status={:?}",
            name,
            status.app_id(),
            status.status()
        );

        /*所有状态下都返回对应的dir
        if status.status() != AppLocalStatusCode::Running {
            let msg = format!(
                "app not running! name={}, id={}, status={:?}",
                name,
                status.app_id(),
                status.status()
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }
        */

        Ok(status.web_dir().map(|v| v.to_owned()))
    }

    pub async fn get_app_local_status(&self, name: &str) -> BuckyResult<Option<AppLocalStatus>> {
        let dec_id = match Self::try_as_object_id(name) {
            Some(id) => Some(id),
            None => {
                // 首先从缓存查询
                if let Some(v) = self.get_app_from_cache(name) {
                    Some(v)
                } else {
                    self.search_app_by_id(name).await?
                }
            }
        };

        if dec_id.is_none() {
            warn!("search app by name but not found! name={}", name);
            return Ok(None);
        }

        let status = self.get_local_status(dec_id.unwrap()).await?;
        if status.is_none() {
            return Ok(None);
        }

        Ok(status)
    }

    fn try_as_object_id(name: &str) -> Option<DecAppId> {
        if name.len() > 40 && name.len() <= 50 {
            match DecAppId::from_str(name) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn get_app_from_cache(&self, name: &str) -> Option<DecAppId> {
        let cache = self.cache.lock().unwrap();
        cache.get(name).map(|v| v.to_owned())
    }

    fn cache_app(&self, name: &str, dec_id: DecAppId) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(name.to_owned(), dec_id);
    }

    // 获取dec_app的状态
    async fn get_local_status(&self, dec_id: DecAppId) -> BuckyResult<Option<AppLocalStatus>> {
        // TODO 改成从objectMap获取
        // 计算对应的AppLocalStatus的id
        let zone = self.zone_manager.get_current_zone().await?;
        let owner = zone.owner().to_owned();

        let dec_status = AppLocalStatus::create(owner, dec_id.clone());
        let status_id = dec_status.desc().calculate_id();
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: NONProtocol::Native,
            object_id: status_id.clone(),
            source: self.device_id.clone(),
        };

        match self.noc.get_object(&noc_req).await {
            Ok(Some(resp)) => {
                assert!(resp.object.is_some());
                assert!(resp.object_raw.is_some());

                let buf = resp.object_raw.unwrap();
                match AppLocalStatus::raw_decode(&buf) {
                    Ok((app, _)) => Ok(Some(app)),
                    Err(e) => {
                        error!(
                            "decode AppLocalStatus object error: dec={}, status={}, {}",
                            dec_id, status_id, e
                        );
                        Err(e)
                    }
                }
            }
            Ok(None) => {
                let msg = format!(
                    "noc get app local status but not found: dec={}, status={}",
                    dec_id, status_id
                );
                info!("{}", msg);
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    // 查找dec_app列表，找到id->dec_app_id
    async fn search_app_by_id(&self, name: &str) -> BuckyResult<Option<DecAppId>> {
        // 过滤noc里面管理的所有的zone对象
        let mut filter = NamedObjectCacheSelectObjectFilter::default();
        filter.obj_type = Some(CoreObjectType::DecApp.into());

        let mut opt = NamedObjectCacheSelectObjectOption {
            page_size: 128,
            page_index: 0,
        };

        loop {
            let noc_req = NamedObjectCacheSelectObjectRequest {
                protocol: NONProtocol::Native,
                source: self.device_id.clone(),
                filter: filter.clone(),
                opt: Some(opt.clone()),
            };
            let obj_list = self.noc.select_object(&noc_req).await.map_err(|e| {
                error!("load dec app objects from noc failed! {}", e);
                e
            })?;
            let ret_count = obj_list.len();

            for obj_info in obj_list {
                let buf = obj_info.object_raw.unwrap();
                let dec_app_id: DecAppId = obj_info.object_id.try_into().unwrap();
                match DecApp::raw_decode(&buf) {
                    Ok((app, _)) => {
                        if self.is_dec_app_match(&app, name) {
                            info!("got dec app object by id: id={}, app={}", name, dec_app_id);

                            self.cache_app(name, dec_app_id.clone());

                            return Ok(Some(dec_app_id));
                        }
                    }
                    Err(e) => {
                        error!("decode dec_app object error: app={}, {}", dec_app_id, e);
                    }
                }
            }

            // 尝试继续下一页的查询
            if ret_count < opt.page_size as usize {
                break;
            }
            opt.page_index += 1;
        }

        Ok(None)
    }

    fn is_dec_app_match(&self, dec_app: &DecApp, name: &str) -> bool {
        // TODO 模糊匹配
        dec_app.name() == name
    }
}
