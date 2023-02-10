use super::requestor::*;
use cyfs_base::*;

use http_types::{Request, Response};
use std::sync::Arc;
use surf::{Client, Config};

/*
use once_cell::sync::Lazy;
static GLOBAL_CLIENT: Lazy<Arc<Client>> = Lazy::new(|| {
    let client = Config::new().try_into().unwrap();

    Arc::new(client)
});
*/

const DEFAULT_MAX_CONNECTIONS_PER_HOST: usize = 50;

#[derive(Clone)]
pub struct SurfHttpRequestor {
    service_addr: String,
    client: Arc<Client>,
}

impl SurfHttpRequestor {
    pub fn new(service_addr: &str, mut http_max_connections_per_host: usize) -> Self {
        if http_max_connections_per_host == 0 {
            http_max_connections_per_host = DEFAULT_MAX_CONNECTIONS_PER_HOST;
        }

        let client = Config::new()
            .set_max_connections_per_host(http_max_connections_per_host)
            .try_into()
            .unwrap();

        Self {
            service_addr: service_addr.to_owned(),
            client: Arc::new(client),
        }
    }
}

#[async_trait::async_trait]
impl HttpRequestor for SurfHttpRequestor {
    async fn request_ext(
        &self,
        req: &mut Option<Request>,
        _conn_info: Option<&mut HttpRequestConnectionInfo>,
    ) -> BuckyResult<Response> {
        debug!(
            "will http request to {}, url={}",
            self.remote_addr(),
            req.as_ref().unwrap().url()
        );

        let begin = std::time::Instant::now();
        match self.client.send(req.take().unwrap()).await {
            Ok(resp) => {
                info!(
                    "http request to {} success! during={}ms",
                    self.remote_addr(),
                    begin.elapsed().as_millis()
                );
                Ok(resp.into())
            }
            Err(e) => {
                let msg = format!(
                    "http request to {} failed! during={}ms, {}",
                    self.remote_addr(),
                    begin.elapsed().as_millis(),
                    e,
                );
                error!("{}", msg);
                Err(BuckyError::from(msg))
            }
        }
    }

    fn remote_addr(&self) -> String {
        self.service_addr.to_string()
    }

    fn remote_device(&self) -> Option<DeviceId> {
        None
    }

    fn clone_requestor(&self) -> Box<dyn HttpRequestor> {
        Box::new(self.clone())
    }

    async fn stop(&self) {
        // self.client.st
        // do nothing
    }
}
