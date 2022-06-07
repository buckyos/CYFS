use cyfs_base::*;
use http_types::{Request, Response, Method, Url};
use async_std::io::{Read, Write};
use log::*;

pub async fn process_req<RW>(req: Request, stream: RW) -> BuckyResult<Response>
    where
    RW: Read + Write + Send + Sync + Unpin + 'static,
{
    let resp = cyfs_util::async_h1_helper::connect_timeout(stream, req,std::time::Duration::from_secs(60 * 5)).await.map_err(|e| {
        error!("read resp from stream error: {}", e);
        e
    })?;

    if resp.status().is_success() {
        Ok(resp)
    } else {
        warn!("resp status: {}", resp.status());
        
        Err(BuckyError::from(resp.status()))
    }
}

pub fn create_post_request<T>(endpoint:String, func:&str, t:& T) -> BuckyResult<Request> 
    where 
    T: RawEncode
{
    let uri = format!("{}/{}", endpoint, func);
    let url = Url::parse(uri.as_ref()).unwrap();
    let mut req = Request::new(Method::Post, url);
    let content = t.to_vec()?;

    req.set_body(content);
    Ok(req)
}
