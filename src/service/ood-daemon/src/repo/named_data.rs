use super::manager::{Repo, RepoPackageInfo, RepoType};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, DirId};
use cyfs_client::{NamedCacheClient, NamedCacheClientConfig};

use async_std::fs::File;
use async_std::io::prelude::*;
use async_trait::async_trait;
use once_cell::sync::OnceCell;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub struct NamedDataRepo {
    client: OnceCell<Arc<NamedCacheClient>>,
}

impl NamedDataRepo {
    pub async fn new() -> BuckyResult<NamedDataRepo> {
        let repo = NamedDataRepo {
            client: OnceCell::new(),
        };

        match repo.init().await {
            Ok(_) => Ok(repo),
            Err(e) => Err(e),
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        // service desc确保它有固定外网地址，连接不走sn。这里sn_list就可以传None
        let mut config = NamedCacheClientConfig::default();
        config.retry_times = 1;
        config.timeout = Duration::from_secs(15 * 60);
        config.tcp_file_manager_port = 5312;
        config.tcp_chunk_manager_port = 5310;
        config.conn_strategy = cyfs_client::ConnStrategy::TcpFirst;
        let mut client = NamedCacheClient::new(config);
        if let Err(e) = client.init().await {
            let msg = format!("init named cache client for repo failed! err={}", e);
            error!("{}", msg);

            return Err(BuckyError::new(e.code(), msg));
        }

        if let Err(_) = self.client.set(Arc::new(client)) {
            unreachable!("init should not call twice!");
        }

        Ok(())
    }

    async fn fetch_inner(
        client: Arc<NamedCacheClient>,
        info: &RepoPackageInfo,
        local_file: &Path,
    ) -> BuckyResult<()> {
        info!(
            "will download pkg from named_data, info={:?}, local={}",
            info,
            local_file.display()
        );

        // 根据是不是dir，来选择不同的接口
        if let Some(inner_path) = &info.inner_path {
            let fid = DirId::from_str(&info.fid).map_err(|e| {
                let msg = format!("invalid named data dir id! info={:?}, err={}", info, e);
                error!("{}", msg);
                BuckyError::new(e.code(), msg)
            })?;

            let inner_path = inner_path.to_owned();
            let local_file = local_file.to_owned();

            client
                .get_dir_by_obj(&fid.object_id(), None, Some(&inner_path), &local_file)
                .await
                .map_err(|e| {
                    let msg = format!("download named data error! info={:?}, err={}", info, e);
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;
        } else {
            let mut dest_file = File::create(local_file).await.map_err(|e| {
                let msg = format!(
                    "open local file error! file={}, err={}",
                    local_file.display(),
                    e
                );

                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            client
                .get_file_by_id(&info.fid, None, &mut dest_file)
                .await
                .map_err(|e| {
                    let msg = format!("download named data error! info={:?}, err={}", info, e);
                    error!("{}", msg);
                    BuckyError::new(e.code(), msg)
                })?;

            if let Err(e) = dest_file.flush().await {
                error!(
                    "flush to dest file error! info={:?}, file={}, {}",
                    info,
                    local_file.display(),
                    e
                );
            }
        }

        info!(
            "download pkg from named_data success! info={:?}, local={}",
            info,
            local_file.display()
        );

        Ok(())
    }
}

#[async_trait]
impl Repo for NamedDataRepo {
    fn get_type(&self) -> RepoType {
        RepoType::NamedData
    }

    async fn fetch(&self, info: &RepoPackageInfo, local_file: &Path) -> BuckyResult<()> {
        use rand::Rng;

        // Use a random retry interval
        let mut retry_interval_secs: u64 = rand::thread_rng().gen_range(60, 60 * 10);
        let mut retry_count = 0;
        loop {
            let info = info.to_owned();
            let local_file = local_file.to_owned();
            let client = self.client.get().unwrap().clone();

            match Self::fetch_inner(client, &info, &local_file).await {
                Ok(()) => break Ok(()),
                Err(e) => {
                    async_std::task::sleep(std::time::Duration::from_secs(retry_interval_secs))
                        .await;
                    retry_interval_secs *= 2;
                    retry_count += 1;

                    if retry_count > 3 {
                        break Err(e);
                    }
                }
            }
        }
    }
}
