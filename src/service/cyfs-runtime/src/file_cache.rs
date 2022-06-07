use cyfs_base::*;
use cyfs_util::async_read_to_sync;
use cyfs_lib::RequestorHelper;

use serde::Serialize;
use std::path::{Path, PathBuf};

const CONTENT_TYPE: &str = "Content-Type";
const CONTENT_LENGTH: &str = "Content-Length";

#[derive(Serialize)]
struct CacheFileInfo {
    path: String,
    hash: String,
    length: u64,
}

#[derive(Clone)]
pub struct FileCacheRecevier {
    cache_dir: PathBuf,
}

impl FileCacheRecevier {
    pub fn new() -> Self {
        let cache_dir = cyfs_util::get_cyfs_root_path()
            .join("chunk_cache")
            .join("file");

        Self { cache_dir }
    }

    fn init_header<State>(
        name: &str,
        headers: &mut hyper::header::Headers,
        req: &tide::Request<State>,
    ) -> BuckyResult<()> {
        let v = req.header(name.to_lowercase().as_str());
        if v.is_none() {
            let msg = format!("{} header not found!", name);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let value = v.unwrap();
        let value: Vec<Vec<u8>> = value
            .iter()
            .map(|v| v.as_str().as_bytes().to_owned())
            .collect();
        headers.set_raw(name.to_owned(), value);

        Ok(())
    }

    async fn process_request<State>(
        &self,
        mut req: tide::Request<State>,
    ) -> BuckyResult<Vec<CacheFileInfo>> {
        /*
        for (name, value) in req.iter() {
            println!("log header {}={}", name.as_str(), value.last().as_str());
        }
        */

        let mut headers = hyper::header::Headers::new();

        Self::init_header(CONTENT_TYPE, &mut headers, &req)?;
        Self::init_header(CONTENT_LENGTH, &mut headers, &req)?;

        /*
        let body = req.body_bytes().await.map_err(|e| {
            let msg = format!("recv body bytes error! {}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let mut reader = std::io::Cursor::new(body);
        */

        let mut reader = async_read_to_sync(req.take_body());
        let data = formdata::read_formdata(&mut reader, &headers).map_err(|e| {
            use std::error::Error;

            // TODO Error的display和to_string实现有问题，会导致异常崩溃，所以这里只能暂时使用description来输出一些描述信息
            #[allow(deprecated)]
            let msg = format!("parse body formdata error! {:?}", e.description());
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // 解析文件列表
        let mut list = vec![];
        for (name, value) in data.files {
            info!(
                "recv cache file: name={}, path={}, len={:?}",
                name,
                value.path.display(),
                value.size
            );
            let info = self.save_file_to_cache(&name, &value.path).await?;
            list.push(info);
        }

        Ok(list)
    }

    async fn save_file_to_cache(&self, name: &str, tmp_path: &Path) -> BuckyResult<CacheFileInfo> {
        if !self.cache_dir.is_dir() {
            if let Err(e) = async_std::fs::create_dir_all(&self.cache_dir).await {
                let msg = format!(
                    "create cache file dir error! {}, {}",
                    self.cache_dir.display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }
        }

        let (hash, len) = cyfs_base::hash_file(&tmp_path).await?;
        let hash = hash.to_hex_string();
        let file_path = self.cache_dir.join(&hash);
        if file_path.exists() {
            warn!(
                "cache file but already exists! name={}, hash={}, len={}",
                name, hash, len
            );
            if let Err(e) = async_std::fs::remove_file(&tmp_path).await {
                error!("remove tmp file error! {}, {}", tmp_path.display(), e);
            }
        } else {
            if let Err(e) = async_std::fs::rename(&tmp_path, &file_path).await {
                let msg = format!(
                    "copy cache file error! {} -> {}, {}",
                    tmp_path.display(),
                    file_path.display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
            }

            info!(
                "cache file success! name={}, file={}, len={}",
                name,
                file_path.display(),
                len
            );
        }

        let info = CacheFileInfo {
            path: file_path.to_string_lossy().to_string(),
            hash,
            length: len,
        };

        Ok(info)
    }
}

#[async_trait::async_trait]
impl<State> tide::Endpoint<State> for FileCacheRecevier
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = match self.process_request(req).await {
            Ok(list) => {
                let mut resp = tide::Response::new(http_types::StatusCode::Ok);
                resp.set_content_type("application/json");
                let s = serde_json::to_string(&list).unwrap();
                resp.set_body(s);
                resp
            }
            Err(e) => RequestorHelper::trans_error(e),
        };

        Ok(resp)
    }
}
