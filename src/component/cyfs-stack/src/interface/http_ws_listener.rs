use super::http_server::{HttpRequestSource, HttpServerHandlerRef};
use super::ObjectListener;
use cyfs_base::*;
use cyfs_lib::*;

use async_std::io::ReadExt;
use async_trait::async_trait;
use std::net::SocketAddr;
use std::sync::{atomic::AtomicU64, Arc};

#[derive(Clone)]
pub(super) struct ObjectHttpWSService {
    // 当前bdt协议栈的device_id
    device_id: DeviceId,

    server_addr: SocketAddr,

    server: HttpServerHandlerRef,
    seq: Arc<AtomicU64>,
}

#[async_trait]
impl ObjectListener for ObjectHttpWSService {
    fn get_protocol(&self) -> RequestProtocol {
        RequestProtocol::HttpLocal
    }

    fn get_addr(&self) -> SocketAddr {
        self.server_addr.clone()
    }

    async fn start(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn stop(&self) -> BuckyResult<()> {
        unreachable!();
    }

    async fn restart(&self) -> BuckyResult<()> {
        Ok(())
    }
}

impl ObjectHttpWSService {
    pub fn new(server_addr: SocketAddr, device_id: DeviceId, server: HttpServerHandlerRef) -> Self {
        let ret = Self {
            server_addr,
            device_id,
            server,
            seq: Arc::new(AtomicU64::new(0)),
        };

        ret
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub async fn process_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        request: Vec<u8>,
    ) -> BuckyResult<Vec<u8>> {
        let seq = self.next_seq();
        let sid = session_requestor.sid();

        debug!(
            "starting recv new ws http request from {}, seq={}",
            sid, seq
        );

        let begin = std::time::Instant::now();

        // 解码request
        let request_reader = async_std::io::Cursor::new(request);
        let (mut req, _body) = async_h1::server::decode(request_reader)
            .await
            .map_err(|e| {
                let msg = format!(
                    "decode http request from buffer error! sid={}, seq={}, {}",
                    sid, seq, e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?
            .ok_or_else(|| {
                let msg = format!(
                    "decode http request from buffer but got none! sid={}, seq={}",
                    sid, seq
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

        // http请求都是同机请求，需要设定为当前device
        req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, self.device_id.to_string());

        let session = session_requestor.session();
        if session.is_none() {
            let msg = format!(
                "ws http request but session is closed, sid={}, seq={}",
                sid, seq
            );
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::ConnectionReset, msg));
        }

        let remote = session.unwrap().conn_info().1.to_owned();
        let source = HttpRequestSource::Local(remote);

        let method = req.method();
        match self.server.respond(source, req).await {
            Ok(resp) => {
                // response编码到buffer
                let mut encoder = async_h1::server::Encoder::new(resp, method);
                let mut buf = vec![];
                if let Err(e) = encoder.read_to_end(&mut buf).await.map_err(|e| {
                    let msg = format!(
                        "read http response to buffer error! sid={}, seq={}, during={}ms, {}",
                        sid,
                        seq,
                        begin.elapsed().as_millis(),
                        e
                    );
                    error!("{}", msg);

                    BuckyError::from(e)
                }) {
                    // read resp error, should return the error with new resp
                    let resp = RequestorHelper::trans_error(e);
                    let mut encoder = async_h1::server::Encoder::new(resp, method);
                    buf.clear();
                    encoder.read_to_end(&mut buf).await.map_err(|e| {
                        let msg = format!(
                            "encode http error response to buffer error! sid={}, seq={}, during={}ms, {}",
                            sid,
                            seq,
                            begin.elapsed().as_millis(),
                            e
                        );
                        error!("{}", msg);
    
                        BuckyError::new(BuckyErrorCode::IoError, msg)
                    })?;
                }

                info!(
                    "ws http request complete! sid={}, seq={}, during={}ms",
                    sid,
                    seq,
                    begin.elapsed().as_millis()
                );

                Ok(buf)
            }
            Err(e) => {
                let msg = format!(
                    "ws http request error, sid={}, seq={}, during={}ms, err={}",
                    sid,
                    seq,
                    begin.elapsed().as_millis(),
                    e
                );
                warn!("{}", msg);

                Err(BuckyError::from(msg))
            }
        }
    }
}
