use cyfs_base::*;
use cyfs_core::*;
use log::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml;

/*
[info]
install_version = "1.0.3"
*/

#[derive(Deserialize, Serialize)]
struct InfoNode {
    install_version: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct Detail {
    info: InfoNode,
}

pub struct AppInstallDetail {
    detail: Detail,
    app_id: String,
}

impl AppInstallDetail {
    pub fn new(app_id: &DecAppId) -> Self {
        let mut ret = Self {
            detail: Detail {
                info: InfoNode {
                    install_version: None,
                },
            },
            app_id: app_id.to_string(),
        };

        let _ = ret.load();

        ret
    }

    pub fn get_install_version(&self) -> Option<String> {
        self.detail
            .info
            .install_version
            .as_ref()
            .map_or(None, |v| Some(v.to_owned()))
    }

    pub fn set_install_version(&mut self, version: Option<&str>) -> BuckyResult<()> {
        if self.detail.info.install_version.is_some()
            && version.is_some()
            && self.detail.info.install_version.as_ref().unwrap() == version.unwrap()
        {
            return Ok(());
        }
        if self.detail.info.install_version.is_none() && version.is_none() {
            return Ok(());
        }

        self.detail.info.install_version = version.map_or(None, |v| Some(v.to_owned()));

        info!("save install detail, {}, {:?}", self.app_id, version);

        self.save()
    }

    fn save(&self) -> BuckyResult<()> {
        let content = toml::to_string(&self.detail).map_err(|e| {
            let msg = format!("format app install detail failed! err={}", e);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let info_file = self.get_detail_file_path()?;

        std::fs::write(&info_file, &content).map_err(|e| {
            let msg = format!(
                "save app install detail failed! file={}, err={}",
                info_file.display(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }

    fn load(&mut self) -> BuckyResult<()> {
        let info_file = self.get_detail_file_path()?;

        if !info_file.is_file() {
            return Ok(());
        }

        let contents = std::fs::read_to_string(&info_file).map_err(|e| {
            let msg = format!(
                "read app install detail failed! file={}, err={}",
                info_file.display(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        self.detail = toml::from_str(&contents).map_err(|e| {
            let msg = format!(
                "parse app install detail failed! file={}, content={}, err={}",
                info_file.display(),
                contents,
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(())
    }

    fn get_detail_file_path(&self) -> BuckyResult<PathBuf> {
        let dir = cyfs_util::get_cyfs_root_path().join("app").join("config");

        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }

        Ok(dir.join(&self.app_id))
    }
}
