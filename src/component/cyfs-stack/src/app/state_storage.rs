use cyfs_base::*;


use serde::{Deserialize, Serialize};
use std::path::PathBuf;


#[derive(Serialize, Deserialize)]
pub(crate) struct AppLocalStateSavedData {
    pub name: String,
    pub dec_id: String,
    pub ip: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AppListSavedData {
    pub gateway_ip: Option<String>,
    pub list: Vec<AppLocalStateSavedData>,
}

pub(crate) struct AppLocalStateStorage {
    file: PathBuf,
}

impl AppLocalStateStorage {
    pub fn new(config_isolate: Option<String>) -> Self {
        let mut file = cyfs_util::get_cyfs_root_path().join("etc");
        if let Some(isolate) = &config_isolate {
            if isolate.len() > 0 {
                file.push(isolate.as_str());
            }
        }

        file.push("app-manager");
        file.push("app-auth-state.toml");

        Self {
            file,
        }
    }

    pub async fn load(&self) -> BuckyResult<Option<AppListSavedData>> {
        if !self.file.exists() {
            return Ok(None);
        }

        let value = async_std::fs::read_to_string(&self.file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load app auth state from config error! file={}, {}",
                    self.file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info!(
            "will load app auth state config: file={}, {}",
            self.file.display(),
            value
        );

        let data: AppListSavedData = toml::from_str(&value).map_err(|e| {
            let msg = format!(
                "invalid app auth state config! file={}, {}",
                self.file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(Some(data))
    }

    pub async fn save(&self, data: AppListSavedData) -> BuckyResult<()> {

        if !self.file.exists() {
            let dir = self.file.parent().unwrap();
            if !dir.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    error!(
                        "create app auth state config dir error! dir={}, {}",
                        dir.display(),
                        e
                    );
                }
            }
        }

        let data = toml::to_string(&data).unwrap();
        async_std::fs::write(&self.file, &data).await.map_err(|e| {
            let msg = format!(
                "write app auth state to config file error! file={}, {}, {}",
                self.file.display(),
                data,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "save app auth state to config file success! file={}, {}",
            self.file.display(),
            data
        );

        Ok(())
    }
}
