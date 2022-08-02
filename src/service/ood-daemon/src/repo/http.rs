use super::manager::RepoPackageInfo;
use crate::repo::Repo;
use crate::repo::RepoType;
use cyfs_base::*;

use async_std::io::WriteExt;
use async_std::net::TcpStream;
use async_trait::async_trait;
use http_types::{Method, Request, Response, Url};
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

pub struct HttpRepo {
    url: Url,
}

impl HttpRepo {
    pub fn new(repo_url: &str) -> BuckyResult<HttpRepo> {
        let url = Url::parse(repo_url).map_err(|e| {
            let msg = format!("invalid http repo url: {}, {}", repo_url, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        Ok(HttpRepo { url })
    }

    async fn resolve_host(&self) -> BuckyResult<SocketAddr> {
        let host = self.url.host();
        if host.is_none() {
            let msg = format!("invalid http repo url host: {}", self.url);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let host = host.unwrap();
        let addr = match host {
            http_types::url::Host::Ipv4(addr) => IpAddr::V4(addr),
            http_types::url::Host::Ipv6(addr) => IpAddr::V6(addr),
            http_types::url::Host::Domain(domain) => {
                let msg = format!(
                    "unsupport http repo url host: url={}, domain={}",
                    self.url, domain
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        let addr = SocketAddr::new(addr, self.url.port().unwrap_or(80));
        Ok(addr)
    }

    async fn request(&self, full_file_name: &str) -> BuckyResult<Response> {
        let host = self.resolve_host().await?;
        let stream = TcpStream::connect(host).await.map_err(|e| {
            let msg = format!("connect to http repo server failed! host={}, {}", host, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ConnectFailed, msg)
        })?;

        let url = self.url.join(full_file_name).map_err(|e| {
            let msg = format!("unsupport http repo url: {}, {}", self.url, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        info!("will request via http reqo url: {}", url);

        let req = Request::new(Method::Get, url.clone());
        let res = async_h1::connect(stream.clone(), req).await.map_err(|e| {
            let msg = format!("http request via http repo failed! host={}, {}", url, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        Ok(res)
    }
}

#[async_trait]
impl Repo for HttpRepo {
    async fn fetch(&self, info: &RepoPackageInfo, local_file: &Path) -> BuckyResult<()> {
        let full_file_name = if let Some(inner_path) = &info.inner_path {
            format!("{}/{}", info.fid, inner_path)
        } else {
            info.fid.clone()
        };

        let mut response = self.request(&full_file_name).await?;

        let content_len = response.len();
        if content_len.is_none() {
            warn!("repo http response content length had not beed set!");
        }

        let mut body = response.take_body().into_reader();

        let mut file = async_std::fs::File::create(local_file).await.map_err(|e| {
            let msg = format!(
                "create local file error! file={}, {}",
                local_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let write_len = async_std::io::copy(&mut body, &mut file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "write response to local file error! file={}, {}",
                    local_file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        file.flush().await.map_err(|e| {
            let msg = format!(
                "flush local file error! file={}, {}",
                local_file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        match content_len {
            Some(v) => {
                if write_len != v as u64 {
                    let msg = format!("read bytes from repsonse but got unmatch content length! file={}, write={}, content={}", local_file.display(), write_len, v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
                }
            }
            None => {}
        }

        Ok(())
    }

    fn get_type(&self) -> RepoType {
        return RepoType::Http;
    }
}
