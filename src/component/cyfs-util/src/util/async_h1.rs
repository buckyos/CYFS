pub mod async_h1_helper {
    use async_std::io::{Read, Write};
    use http_types::{Request, Response};

    /// Opens an HTTP/1.1 connection to a remote host with timeout.
    pub async fn connect_timeout<RW>(stream: RW, req: Request, dur: std::time::Duration) -> http_types::Result<Response>
    where
        RW: Read + Write + Send + Sync + Unpin + 'static,
    {
        // CYFS LOG
        match async_std::future::timeout(dur, async_h1::connect(stream, req)).await {
            Ok(ret) => {
                ret
            }
            Err(async_std::future::TimeoutError { .. }) => {
                let msg = format!("http request timeout! dur={:?}", dur);
                log::error!("{}", msg);
                Err(http_types::Error::from_str(http_types::StatusCode::GatewayTimeout, msg))
            }
        }
    }
}