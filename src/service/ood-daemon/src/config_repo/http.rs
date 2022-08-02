use super::DeviceConfigRepo;
use crate::repo::HttpRepoBase;
use cyfs_base::*;

use async_trait::async_trait;

pub struct DeviceConfigHttpRepo {
    repo: HttpRepoBase,
}

impl DeviceConfigHttpRepo {
    pub fn new(repo_url: &str) -> BuckyResult<Self> {
        Ok(Self {
            repo: HttpRepoBase::new(repo_url)?,
        })
    }
}

#[async_trait]
impl DeviceConfigRepo for DeviceConfigHttpRepo {
    fn get_type(&self) -> &'static str {
        "http"
    }

    async fn fetch(&self) -> BuckyResult<String> {
        let mut response = self.repo.request("device-config.toml").await?;
        if !response.status().is_success() {
            let msg = format!(
                "fetch device config from http repo failed! status={}, url={}",
                response.status(),
                self.repo.url(),
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        let buf = response.take_body().into_string().await.map_err(|e| {
            let msg = format!(
                "fetch device config response body string from http repo failed! url={}, {}",
                self.repo.url(),
                e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        Ok(buf)
    }
}
