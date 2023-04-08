use std::path::PathBuf;
use std::time::Duration;
use app_manager::package::AppPackage;
use app_manager_lib::AppManagerConfig;
use cyfs_base::{AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, FileDecoder, FileEncoder, NamedObject, ObjectId, OwnerObjectDesc, RawFrom};
use cyfs_base_meta::SavedMetaObject;
use cyfs_client::{NamedCacheClient, NamedCacheClientConfig};
use cyfs_core::{AppList, APPLIST_APP_CATEGORY, AppListObj, AppStatus, AppStatusObj, DecApp, DecAppId, DecAppObj};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_util::get_cyfs_root_path;
use crate::asset::{OODAsset};

pub(crate) struct AppRepoDownloader {
    repo_path: PathBuf,
    client: NamedCacheClient,
    meta_client: MetaClient
}

impl AppRepoDownloader {
    pub fn new() -> Self {
        let mut config = NamedCacheClientConfig::default();
        config.retry_times = 3;
        config.timeout = Duration::from_secs(10*60);
        config.tcp_file_manager_port = 5312;
        config.tcp_chunk_manager_port = 5310;
        config.conn_strategy = cyfs_client::ConnStrategy::TcpFirst;
        Self {
            repo_path: get_cyfs_root_path().join("app_repo"),
            client: NamedCacheClient::new(config),
            meta_client: MetaClient::new_target(MetaMinerTarget::default())
        }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        if let Err(e) = self.client.init().await {
            let msg = format!("init named cache client for repo failed! err={}", e);
            error!("{}", msg);

            return Err(BuckyError::new(e.code(), msg));
        }

        let known_sn = cyfs_util::get_sn_desc().iter().map(|(_, device)| {
            device.clone()
        }).collect();
        let _ = self.client.reset_known_sn_list(known_sn);

        Ok(())
    }
    pub async fn download_app(&self, id: &DecAppId, ver: &str) -> BuckyResult<()> {
        info!("download app {} version {}", id.object_id(), ver);
        let dec_app = if let SavedMetaObject::Data(data) = self.meta_client.get_desc(id.object_id()).await? {
            DecApp::clone_from_slice(&data.data)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, format!("app {} from meta type mismatch!", &id.object_id())))
        }?;

        let owner = dec_app.desc().owner().as_ref().ok_or(BuckyError::from(BuckyErrorCode::InvalidInput))?;
        let dir = dec_app.find_source(ver)?;

        let target_path = self.repo_path.join(dir.to_string());
        AppPackage::download(&dir, owner, &self.client, &target_path).await.map_err(|e| {
            error!("download app {} dir {} err {}",id.object_id(), &dir, e);
            e
        })
    }

    pub async fn download(&self, asset: &OODAsset) -> BuckyResult<()> {
        asset.extract_app_repo()?;
        asset.extract_app_manager()?;

        let sys_app_list_id = self.get_sys_app_list_id()?;
        info!("try get sys app list {}", sys_app_list_id);

        let mut list = if let SavedMetaObject::Data(data) = self.meta_client.get_desc(&sys_app_list_id).await? {
            AppList::clone_from_slice(&data.data)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, format!("app list {} from meta type mismatch!", &sys_app_list_id)))
        }?;

        let app_config = AppManagerConfig::load();
        let owner = list.desc().owner().as_ref().unwrap().clone();
        for id in &app_config.app.include {
            if let Ok(SavedMetaObject::Data(data)) = self.meta_client.get_desc(id.object_id()).await {
                if let Ok(app) = DecApp::clone_from_slice(&data.data) {
                    if let Ok(latest) = app.find_tag("latest") {
                        info!("add include app {} ver {}", id, latest);
                        list.put(AppStatus::create(owner.clone(), id.clone(), latest.to_owned(), true));
                    }
                }
            }
        }

        for (id, status) in list.app_list() {
            self.download_app(id, status.version()).await?
        }

        list.encode_to_file(&self.repo_path.join("app_list.obj"), false)?;

        info!("sync app repo from {} success!", &sys_app_list_id);

        Ok(())
    }

    fn get_sys_app_list_id(&self) -> BuckyResult<ObjectId> {
        let mut repo_path = get_cyfs_root_path();
        repo_path.push("etc");
        repo_path.push("desc");
        repo_path.push("app_repo.desc");
        let (obj, _) = AnyNamedObject::decode_from_file(&repo_path, &mut vec![])?;
        let id = obj.calculate_id();

        Ok(AppList::generate_id(id.clone(), "", APPLIST_APP_CATEGORY))

    }
}