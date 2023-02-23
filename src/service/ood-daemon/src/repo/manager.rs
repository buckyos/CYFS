use super::http::HttpRepo;
use super::local::LocalRepo;
use super::named_data::NamedDataRepo;
use crate::config::PATHS;
use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_util::TomlHelper;

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub enum RepoType {
    Http,
    NamedData,
    Local,
}

#[derive(Debug, Clone)]
pub struct RepoPackageInfo {
    pub file_name: String,
    pub fid: String,
    pub inner_path: Option<String>,
}

impl RepoPackageInfo {
    pub fn new(fid: &str) -> Self {
        let parts: Vec<&str> = fid.split("/").collect();
        let file_name = fid.replace("/", "_");

        let mut inner_path = None;
        let fid = parts[0];
        if parts.len() > 1 {
            inner_path = Some(parts[1..].join("/"));
        }

        Self {
            file_name,
            fid: fid.to_owned(),
            inner_path,
        }
    }
}

#[async_trait]
pub trait Repo: Send + Sync {
    async fn fetch(&self, info: &RepoPackageInfo, local_file: &Path) -> BuckyResult<()>;

    async fn fetch_with_timeout(
        &self,
        info: &RepoPackageInfo,
        local_file: &Path,
        timeout: std::time::Duration,
    ) -> BuckyResult<()> {
        let ret = async_std::future::timeout(timeout, self.fetch(info, local_file)).await;
        match ret {
            Ok(ret) => ret,
            Err(async_std::future::TimeoutError { .. }) => {
                let msg = format!(
                    "fetch from repo timeout: repo={:?}, info={:?}",
                    self.get_type(),
                    info,
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Timeout, msg))
            }
        }
    }

    fn get_type(&self) -> RepoType;
}

pub struct RepoManager {
    cache_dir: PathBuf,
    repo_config: Mutex<Option<String>>,
    repo_list: Mutex<Vec<Arc<Box<dyn Repo>>>>,
}

impl RepoManager {
    fn cache_dir() -> PathBuf {
        let cache_dir = PATHS.repo_cache_root.clone();

        if !cache_dir.exists() {
            info!("will create cache dir: {}", cache_dir.display());
            if let Err(e) = std::fs::create_dir_all(&cache_dir) {
                error!(
                    "create cache dir failed! dir={}, err={}",
                    cache_dir.display(),
                    e
                );
            }
        } else {
            assert!(cache_dir.is_dir());
        }

        cache_dir
    }

    pub fn new() -> RepoManager {
        let cache_dir = Self::cache_dir();
        RepoManager {
            repo_config: Mutex::new(None),
            repo_list: Mutex::new(Vec::new()),
            cache_dir,
        }
    }

    // 直接从named-repo加载，忽略本地加载的repo配置
    pub async fn new_with_named_data() -> BuckyResult<Self> {
        let cache_dir = Self::cache_dir();

        let repo = NamedDataRepo::new().await?;
        let repo = Arc::new(Box::new(repo) as Box<dyn Repo>);

        Ok(Self {
            repo_config: Mutex::new(None),
            repo_list: Mutex::new(vec![repo]),
            cache_dir,
        })
    }

    pub async fn fetch_service(&self, fid: &str) -> BuckyResult<PathBuf> {
        let repo_list = self.repo_list.lock().unwrap().clone();
        if repo_list.is_empty() {
            let msg = format!("fetch service but repo list is empty! fid={}", fid);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
        }

        Self::fetch_service_with_repo_list(&self.cache_dir, fid, &repo_list).await
    }

    async fn fetch_service_with_repo_list(
        cache_dir: &Path,
        fid: &str,
        repo_list: &Vec<Arc<Box<dyn Repo>>>,
    ) -> BuckyResult<PathBuf> {
        let info = RepoPackageInfo::new(fid);

        // 生成本地临时文件
        let local_file = cache_dir.join(&info.file_name);

        // 遍历repo列表，直到找到
        let mut err = None;
        for repo in repo_list {
            // FIXME 如果存在本地文件，是否直接使用？
            // 如果存在本地文件，那么首先尝试删除
            if local_file.exists() {
                //warn!(
                //    "local cache file exists, now will reuse: {}",
                //    local_file.display()
                //);
                //return Ok(local_file);

                if let Err(e) = std::fs::remove_file(&local_file) {
                    error!(
                        "remove file error, file={}, err={}",
                        local_file.display(),
                        e
                    );

                    return Err(BuckyError::from(e));
                }
            }

            match Self::fetch_service_with_repo(&repo, &info, local_file.as_path()).await {
                Ok(_) => {
                    info!(
                        "fetch pkg from repo success! fid={}, repo={:?}",
                        fid,
                        repo.get_type()
                    );
                    return Ok(local_file);
                }
                Err(e) => {
                    err = Some(e);
                }
            }
        }

        let msg = format!(
            "fetch service from repo list failed! fid={}, err={:?}",
            fid, err
        );
        error!("{}", msg);

        Err(err.unwrap_or(BuckyError::from(msg)))
    }

    async fn fetch_service_with_repo(
        repo: &Arc<Box<dyn Repo>>,
        info: &RepoPackageInfo,
        local_file: &Path,
    ) -> BuckyResult<()> {
        if let Err(e) = repo
            .fetch_with_timeout(
                info,
                local_file,
                std::time::Duration::from_secs(60 * 60 * 2),
            )
            .await
        {
            error!(
                "fetch from repo error: repo={:?}, info={:?}, err={}",
                repo.get_type(),
                info,
                e
            );

            return Err(e);
        }

        info!(
            "fetch from repo success: repo={:?}, info={:?}, file={}",
            repo.get_type(),
            info,
            local_file.display()
        );

        Ok(())
    }

    // 从system_config的repo字段加载配置
    pub async fn load(&self, repo_node: &Vec<toml::Value>) -> BuckyResult<()> {
        assert!(repo_node.len() > 0);

        let config_string = serde_json::to_string(&repo_node).unwrap();
        {
            let current = self.repo_config.lock().unwrap();
            if current.as_deref() == Some(&config_string) {
                return Ok(());
            }

            info!("repo config changed! {:?} -> {}", &*current, config_string);
        }

        let mut list = vec![];
        for item in repo_node.iter() {
            info!("new repo item: {:?}", item);
            if let toml::Value::Table(m) = item {
                let repo = Self::load_repo_item(&m).await?;
                list.push(Arc::new(repo));
            } else {
                let msg = format!("unsupport repo item format!");
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        }

        // Support for repeated loading
        {
            let mut current = self.repo_list.lock().unwrap();
            if !current.is_empty() {
                warn!("will replace current repo list!");
            }

            *current = list;
        }

        *self.repo_config.lock().unwrap() = Some(config_string);

        Ok(())
    }

    async fn load_repo_item(repo_item_node: &toml::value::Table) -> BuckyResult<Box<dyn Repo>> {
        let ret = repo_item_node.get("type");
        if ret.is_none() {
            error!("invalid repo config node: {:?}", repo_item_node);
            return Err(BuckyError::from("invalid repo config"));
        }

        let repo_type: String = TomlHelper::decode_string_field(repo_item_node, "type")?;
        match repo_type.as_str() {
            "named_data" => Self::load_named_data_repo(repo_item_node).await,
            "local" => Self::load_local_repo(repo_item_node),
            "http" => Self::load_http_repo(repo_item_node),
            _ => {
                let msg = format!("unknown repo type {}", repo_type);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }

    fn load_local_repo(repo_item_node: &toml::value::Table) -> BuckyResult<Box<dyn Repo>> {
        // local_store是可选项
        let local_store: Option<String> =
            TomlHelper::decode_option_string_field(repo_item_node, "local_store")?;

        info!("load local repo success! local_store={:?}", local_store);

        let local_repo = LocalRepo::new(local_store);
        Ok(Box::new(local_repo))
    }

    async fn load_named_data_repo(
        _repo_item_node: &toml::value::Table,
    ) -> Result<Box<dyn Repo>, BuckyError> {
        match NamedDataRepo::new().await {
            Ok(repo) => Ok(Box::new(repo)),
            Err(e) => Err(e),
        }
    }

    fn load_http_repo(repo_item_node: &toml::value::Table) -> BuckyResult<Box<dyn Repo>> {
        let url: String = TomlHelper::decode_string_field(repo_item_node, "url")?;

        info!("load http repo success! url={:?}", url);

        let http_repo = HttpRepo::new(&url)?;
        Ok(Box::new(http_repo))
    }
}

use lazy_static::lazy_static;

lazy_static! {
    pub static ref REPO_MANAGER: Arc<RepoManager> = {
        return Arc::new(RepoManager::new());
    };
}
