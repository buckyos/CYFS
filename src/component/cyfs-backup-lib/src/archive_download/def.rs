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
            None => self.base_url.clone(),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RemoteArchiveInfo {
    ZipFile(RemoteArchiveUrl),
    Folder(RemoteArchiveUrl),
}

impl RemoteArchiveInfo {
    // Supports URLs in two formats
    // {base_url}?{query_string}
    // {base_url}/${filename}?{query_string}
    pub fn parse(url: &str) -> BuckyResult<Self> {
        let (base_url, query_string) = match url.split_once("?") {
            Some((base_url, query_string)) => (base_url.to_owned(), Some(query_string.to_owned())),
            None => (url.to_owned(), None),
        };

        let ret = match base_url.find("${filename}") {
            Some(_) => {
                let base_url = base_url.replace("${filename}", "");

                let info = RemoteArchiveUrl {
                    base_url,
                    file_name: None,
                    query_string,
                };
                RemoteArchiveInfo::Folder(info)
            }
            None => {
                let info = RemoteArchiveUrl {
                    base_url,
                    file_name: None,
                    query_string,
                };
                RemoteArchiveInfo::ZipFile(info)
            }
        };

        Ok(ret)
    }
}

#[cfg(test)]
mod test {
    use super::RemoteArchiveInfo;

    #[test]
    fn test_url() {
        let url = "http://127.0.0.1:1234/a/b?token=123456";
        let info = RemoteArchiveInfo::parse(url).unwrap();
        if let RemoteArchiveInfo::ZipFile(info) = info {
            assert_eq!(info.base_url, "http://127.0.0.1:1234/a/b");
            assert_eq!(info.query_string.as_deref(), Some("token=123456"));
        } else {
            unreachable!();
        }

        let url = "http://127.0.0.1:1234/a/b";
        let info = RemoteArchiveInfo::parse(url).unwrap();
        if let RemoteArchiveInfo::ZipFile(info) = info {
            assert_eq!(info.base_url, "http://127.0.0.1:1234/a/b");
            assert_eq!(info.query_string, None);
        } else {
            unreachable!();
        }

        let url = "http://127.0.0.1:1234/a/b/${filename}?token=123456";
        let info = RemoteArchiveInfo::parse(url).unwrap();
        if let RemoteArchiveInfo::Folder(info) = info {
            assert_eq!(info.base_url, "http://127.0.0.1:1234/a/b/");
            assert_eq!(info.query_string.as_deref(), Some("token=123456"));
        } else {
            unreachable!();
        }

        let url = "http://127.0.0.1:1234/a/b/${filename}";
        let info = RemoteArchiveInfo::parse(url).unwrap();
        if let RemoteArchiveInfo::Folder(info) = info {
            assert_eq!(info.base_url, "http://127.0.0.1:1234/a/b/");
            assert_eq!(info.query_string, None);
        } else {
            unreachable!();
        }
    }
}