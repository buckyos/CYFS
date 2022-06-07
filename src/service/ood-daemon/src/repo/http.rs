use std::error::Error;
use std::fs::File;
use std::path::Path;

use async_trait::async_trait;
extern crate reqwest;
extern crate url;
use crate::repo::Repo;
use crate::repo::RepoType;
use futures_util::StreamExt;
use log;
use std::io::Write;

pub struct HttpRepo {
    url: String,
}

impl HttpRepo {
    pub fn new(repo_url: &str) -> Result<HttpRepo, BuckyError> {
        let ret = url::Url::parse(repo_url);
        if let Err(e) = ret {
            log::error!("invaid repo url: {}", repo_url);
            return Err(Box::new(e));
        }

        Ok(HttpRepo {
            url: String::from(repo_url),
        })
    }
}

#[async_trait]
impl Repo for HttpRepo {
    async fn fetch(&mut self, pkg_fid: &str, local_file: &Path) -> Result<(), BuckyError> {
        let mut parts = url::Url::parse(&self.url).unwrap();
        parts.set_query(Some(&format!("fid={}", pkg_fid)));
        let pkg_url = parts.into_string();

        log::info!("will download: {} -> {}", pkg_url, local_file.display());

        let create_ret = File::create(local_file);
        if let Err(e) = create_ret {
            error!("open file error: {}, err={}", local_file.display(), e);
            return Err(Box::new(e));
        }

        let out = create_ret.unwrap();

        let get_ret = reqwest::get(pkg_url.as_str()).await;
        if let Err(e) = get_ret {
            error!("get package from url error: {}, err={}", &pkg_url, e);
            return Err(Box::new(e));
        }

        let resp = get_ret.unwrap();

        // 判断状态是否出错
        if !resp.status().is_success() {
            match resp.error_for_status_ref() {
                Err(e) => {
                    error!("get package from url error status: {}, err={}", &pkg_url, e);
                    return Err(Box::new(e));
                }
                Ok(_) => {
                    // 非4xx 5xx的错误，比如重定向等
                    error!(
                        "get package from url error status: {}, err={}",
                        &pkg_url,
                        resp.status().as_str()
                    );
                    return Err(BuckyError::from(resp.status().as_str()));
                }
            }
        }

        let mut writer = std::io::BufWriter::new(out);
        let mut stream = resp.bytes_stream();
        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    if let Err(e) = writer.write(&bytes) {
                        error!(
                            "write to file error, file={}, e={}",
                            local_file.display(),
                            e
                        );
                        return Err(Box::new(e));
                    }
                }
                Err(e) => {
                    error!("stream return error, url={}, e={}", pkg_url, e);
                    return Err(Box::new(e));
                }
            }
        }

        writer.flush()?;

        // io::copy(&mut resp.bytes_stream(), &mut out).expect("failed to copy content");

        Ok(())
    }

    fn get_type(&self) -> RepoType {
        return RepoType::Http;
    }
}
