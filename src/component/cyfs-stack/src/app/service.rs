use crate::front::*;
use crate::root_state_api::*;
use cyfs_base::*;
use cyfs_debug::Mutex;

use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppService {
    device_id: DeviceId,
    cache: Arc<Mutex<HashMap<String, ObjectId>>>,
    root_state: Arc<GlobalStateManager>,
}

impl AppService {
    pub fn new(device_id: DeviceId, root_state: Arc<GlobalStateManager>) -> Self {
        Self {
            device_id,
            root_state,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_app_web_dir(
        &self,
        dec: &FrontARequestDec,
        ver: &FrontARequestVersion,
    ) -> BuckyResult<(ObjectId, ObjectId)> {
        let dec_id = self.get_app(dec).await?;

        let dir_id = self.search_app_web_dir(&dec_id, ver).await?;
        Ok((dec_id, dir_id))
    }

    pub async fn get_app_local_status(
        &self,
        dec: &FrontARequestDec,
    ) -> BuckyResult<(ObjectId, ObjectId)> {
        let dec_id = self.get_app(dec).await?;

        let local_status_id = self.search_local_status(&dec_id).await?;

        Ok((dec_id, local_status_id))
    }

    async fn get_app(&self, dec: &FrontARequestDec) -> BuckyResult<ObjectId> {
        let dec_id = match dec {
            FrontARequestDec::DecID(dec_id) => dec_id.to_owned(),
            FrontARequestDec::Name(name) => self.get_app_by_name(name).await?,
        };

        Ok(dec_id)
    }

    fn get_app_from_cache(&self, name: &str) -> Option<ObjectId> {
        let cache = self.cache.lock().unwrap();
        cache.get(name).map(|v| v.to_owned())
    }

    fn cache_app(&self, name: &str, dec_id: ObjectId) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(name.to_owned(), dec_id);
    }

    // 获取dec_app的状态
    async fn search_local_status(&self, dec_id: &ObjectId) -> BuckyResult<ObjectId> {
        let dec_root_manager = self
            .root_state
            .get_dec_root_manager(cyfs_core::get_system_dec_app().object_id(), false)
            .await?;

        let op_env = dec_root_manager.create_op_env().await?;

        let path = format!("/app/{}/local_status", dec_id.to_string());
        let ret = op_env.get_by_path(&path).await?;
        if ret.is_none() {
            let msg = format!(
                "get app local_status by name but not found! dec={}, path={}",
                dec_id, path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let local_status_id = ret.unwrap();
        info!("get app local_status: {} -> {}", dec_id, local_status_id);

        Ok(local_status_id)
    }

    async fn search_app_web_dir(
        &self,
        dec_id: &ObjectId,
        ver: &FrontARequestVersion,
    ) -> BuckyResult<ObjectId> {
        let ver_seg = match ver {
            FrontARequestVersion::Current | FrontARequestVersion::DirID(_) => "current",
            FrontARequestVersion::Version(ver) => ver.as_str(),
        };

        let dec_root_manager = self
            .root_state
            .get_dec_root_manager(cyfs_core::get_system_dec_app().object_id(), false)
            .await?;
        let op_env = dec_root_manager.create_op_env().await?;

        let path = format!("/app/{}/versions/{}", dec_id, ver_seg);
        let ret = op_env.get_by_path(&path).await?;
        if ret.is_none() {
            let msg = format!(
                "get app dir-id by version but not found! dec-id={}, path={}",
                dec_id, path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let dir_id = ret.unwrap();
        match ver {
            FrontARequestVersion::DirID(id) => {
                if *id != dir_id {
                    let msg = format!(
                        "get app dir-id by version but not match! dec-id={}, current={}, request={}",
                        dec_id, dir_id, id,
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }
            }
            _ => {}
        };

        info!(
            "get app dir-id by version: dec={}, ver={}, dir={}",
            dec_id, ver_seg, dir_id,
        );

        Ok(dir_id)
    }

    async fn get_app_by_name(&self, name: &str) -> BuckyResult<ObjectId> {
        if let Some(dec_id) = self.get_app_from_cache(name) {
            return Ok(dec_id);
        }

        // TODO add failure cache

        self.search_app_by_name(name).await
    }

    // get dec-id by name from /system/app/names/${name}
    async fn search_app_by_name(&self, name: &str) -> BuckyResult<ObjectId> {
        let dec_root_manager = self
            .root_state
            .get_dec_root_manager(cyfs_core::get_system_dec_app().object_id(), false)
            .await?;
        let op_env = dec_root_manager.create_op_env().await?;

        let name_path = format!("/app/names/{}", name);
        let ret = op_env.get_by_path(&name_path).await?;
        if ret.is_none() {
            let msg = format!(
                "get app by name but not found! name={}, path={}",
                name, name_path,
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        info!("get app by name: {} -> {}", name, ret.as_ref().unwrap());

        let dec_id = ret.unwrap();
        self.cache_app(name, dec_id.clone());

        Ok(dec_id)
    }
}
