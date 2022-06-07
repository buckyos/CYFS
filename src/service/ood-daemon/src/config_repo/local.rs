use super::DeviceConfigRepo;
use cyfs_base::*;

use async_trait::async_trait;
use std::path::PathBuf;

pub struct DeviceConfigLocalRepo {
    local_dir: PathBuf,
}

impl DeviceConfigLocalRepo {
    pub fn new() -> DeviceConfigLocalRepo {
        DeviceConfigLocalRepo {
            local_dir: ::cyfs_util::get_cyfs_root_path().join("repo_store"),
        }
    }
}

#[async_trait]
impl DeviceConfigRepo for DeviceConfigLocalRepo {
    fn get_type(&self) -> &'static str {
        "local"
    }

    async fn fetch(&self) -> Result<String, BuckyError> {
        let local_file = self.local_dir.join("device-config.toml");
        if !local_file.is_file() {
            let msg = format!(
                "device_config not found or not valid file in local repo! file={}",
                local_file.display()
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let str = match std::fs::read_to_string(&local_file) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "load device_config from local repo failed! file={}, err={}",
                    local_file.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(str)
    }
}
