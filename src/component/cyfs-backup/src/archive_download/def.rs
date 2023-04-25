use cyfs_base::*;

use http_types::Url;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RemoteArchiveUrl {
    pub base_url: String,
    pub file_name: Option<String>,
    pub query_string: Option<String>,
}

impl RemoteArchiveUrl {
    pub fn parse_url(&self) -> BuckyResult<Url> {
        let url = match &self.file_name {
            Some(file_name) => {
                format!("{}/{}", self.base_url.trim_end_matches('/'), file_name)
            }
            None => {
                self.base_url.clone()
            }
        };

        let mut url = Url::parse(&url).map_err(|e| {
            let msg = format!(
                "invalid remote archive url format! {}, {}",
                self.base_url, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if let Some(query) = &self.query_string {
            url.set_query(Some(query.as_str()));
        }

        Ok(url)
    }
}


pub enum RemoteArchiveInfo {
    ZipFile(RemoteArchiveUrl),
    Folder(RemoteArchiveUrl),
}