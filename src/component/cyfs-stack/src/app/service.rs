use super::cache::AppCache;
use crate::front::*;
use crate::root_state::GlobalStateInputProcessorRef;
use crate::root_state::GlobalStateOutputTransformer;
use crate::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::GlobalStateStub;

pub enum AppInstallStatus {
    Installed((ObjectId, ObjectId)),
    NotInstalled(FrontARequestDec),
}

#[derive(Clone)]
pub struct AppService {
    cache: AppCache,
    root_state_stub: GlobalStateStub,
}

impl AppService {
    pub async fn new(
        zone_manager: &ZoneManagerRef,
        root_state: GlobalStateInputProcessorRef,
    ) -> BuckyResult<Self> {
        let info = zone_manager.get_current_info().await?;
        let source = zone_manager
            .get_current_source_info(&Some(cyfs_core::get_system_dec_app().to_owned()))
            .await?;
        let processor = GlobalStateOutputTransformer::new(root_state, source);
        let root_state_stub = GlobalStateStub::new(
            processor,
            Some(info.zone_device_ood_id.object_id().clone()),
            Some(cyfs_core::get_system_dec_app().to_owned()),
        );

        Ok(Self {
            root_state_stub,
            cache: AppCache::new(),
        })
    }

    pub async fn get_app_web_dir(
        &self,
        dec: &FrontARequestDec,
        ver: &FrontARequestVersion,
        flush_cache: bool,
    ) -> BuckyResult<AppInstallStatus> {
        let dec_id = match self.get_app(dec).await? {
            Some(dec_id) => dec_id,
            None => {
                return Ok(AppInstallStatus::NotInstalled(dec.to_owned()));
            }
        };

        let ret = self.search_app_web_dir(&dec_id, ver, flush_cache).await?;
        let status = match ret {
            Some(dir_id) => AppInstallStatus::Installed((dec_id, dir_id)),
            None => AppInstallStatus::NotInstalled(FrontARequestDec::DecID(dec_id)),
        };

        Ok(status)
    }

    pub async fn get_app_local_status(
        &self,
        dec: &FrontARequestDec,
    ) -> BuckyResult<AppInstallStatus> {
        let dec_id = match self.get_app(dec).await? {
            Some(dec_id) => dec_id,
            None => {
                return Ok(AppInstallStatus::NotInstalled(dec.to_owned()));
            }
        };

        let ret = self.search_local_status(&dec_id).await?;
        let status = match ret {
            Some(local_status_id) => AppInstallStatus::Installed((dec_id, local_status_id)),
            None => AppInstallStatus::NotInstalled(FrontARequestDec::DecID(dec_id)),
        };

        Ok(status)
    }

    async fn get_app(&self, dec: &FrontARequestDec) -> BuckyResult<Option<ObjectId>> {
        let dec_id = match dec {
            FrontARequestDec::DecID(dec_id) => Some(dec_id.to_owned()),
            FrontARequestDec::Name(name) => self.get_app_by_name(name).await?,
        };

        Ok(dec_id)
    }

    // 获取dec_app的状态
    async fn search_local_status(&self, dec_id: &ObjectId) -> BuckyResult<Option<ObjectId>> {
        let op_env = self.root_state_stub.create_path_op_env().await?;

        let path = format!("/app/{}/local_status", dec_id.to_string());
        let ret = op_env.get_by_path(&path).await?;
        let _ = op_env.abort().await;
        if ret.is_none() {
            let msg = format!(
                "get app local_status by name but not found! dec={}, path={}",
                dec_id, path,
            );
            warn!("{}", msg);
            return Ok(None);
        }

        let local_status_id = ret.unwrap();
        info!("get app local_status: {} -> {}", dec_id, local_status_id);

        Ok(Some(local_status_id))
    }

    async fn search_app_web_dir(
        &self,
        dec_id: &ObjectId,
        ver: &FrontARequestVersion,
        flush_cache: bool,
    ) -> BuckyResult<Option<ObjectId>> {
        let ver_seg = match ver {
            FrontARequestVersion::Current => "current",
            FrontARequestVersion::DirID(id) => {
                // FIXME Need to check whether the id is really a dir_id of a certain version of dec?
                return Ok(Some(id.to_owned()));
            }
            FrontARequestVersion::Version(ver) => ver.as_str(),
        };

        if flush_cache {
            self.cache.clear_dir(dec_id, ver);
        }
        
        // First try to get result from cache
        let ret = self.cache.get_dir_by_version(dec_id, ver);
        if let Some(cache) = ret {
            return Ok(cache);
        }

        debug!("will search app web dir: dec={}, ver={:?}", dec_id, ver);

        let op_env = self.root_state_stub.create_path_op_env().await?;

        let path = format!("/app/{}/versions/{}", dec_id, ver_seg);
        let ret = op_env.get_by_path(&path).await?;
        let _ = op_env.abort().await;

        // First cache result
        self.cache.cache_dir_with_version(dec_id, ver, ret.clone());

        if ret.is_none() {
            let msg = format!(
                "get app dir_id by version but not found! dec={}, path={}",
                dec_id, path,
            );
            warn!("{}", msg);
            return Ok(None);
        }

        let dir_id = ret.unwrap();

        /*
        match ver {
            FrontARequestVersion::DirID(id) => {
                if *id != dir_id {
                    let msg = format!(
                        "get app dir-id by version but not match! dec-id={}, current={}, request={}",
                        dec_id, dir_id, id,
                    );
                    warn!("{}", msg);
                    return Ok(None);
                }
            }
            _ => {}
        };
        */

        info!(
            "get app dir-id by version: dec={}, ver={}, dir={}",
            dec_id, ver_seg, dir_id,
        );

        Ok(Some(dir_id))
    }

    async fn get_app_by_name(&self, name: &str) -> BuckyResult<Option<ObjectId>> {
        let ret = self.cache.get_app_by_name(name);
        if let Some(cache) = ret {
            return Ok(cache);
        }

        let ret = self.search_app_by_name(name).await?;
        self.cache.cache_app_with_name(name, ret.clone());

        Ok(ret)
    }

    // get dec-id by name from /system/app/names/${name}
    async fn search_app_by_name(&self, name: &str) -> BuckyResult<Option<ObjectId>> {
        let op_env = self.root_state_stub.create_path_op_env().await?;

        let name_path = format!("/app/names/{}", name);
        let ret = op_env.get_by_path(&name_path).await?;
        let _ = op_env.abort().await;
        if ret.is_none() {
            let msg = format!(
                "get app by name but not found! name={}, path={}",
                name, name_path,
            );
            warn!("{}", msg);
            return Ok(None);
        }

        info!("get app by name: {} -> {}", name, ret.as_ref().unwrap());

        Ok(ret)
    }
}
