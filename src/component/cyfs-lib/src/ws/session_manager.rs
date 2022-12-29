use super::request::*;
use super::session::*;
use cyfs_base::*;
use cyfs_debug::Mutex;

use async_std::io::{Read, Write};
use http_types::Url;
use rand::Rng;
use std::collections::HashMap;
use std::marker::Send;
use std::net::SocketAddr;
use std::sync::Arc;

struct WebSocketSessionManagerInner {
    list: HashMap<u32, Arc<WebSocketSession>>,
    next_sid: u32,
    handler: Option<Box<dyn WebSocketRequestHandler>>,

    // 带状态的select
    next_select_index: usize,
}

impl Drop for WebSocketSessionManagerInner {
    fn drop(&mut self) {
        warn!("ws session manager dropped! next_sid={}", self.next_sid);
    }
}

impl WebSocketSessionManagerInner {
    fn new(handler: Box<dyn WebSocketRequestHandler>) -> Self {
        // sid随机化
        let mut rng = ::rand::thread_rng();
        let sid = loop {
            let ret = rng.gen::<u32>();
            if ret != u32::MAX {
                break ret;
            }
        };

        info!("ws manager sid start at {}", sid);

        Self {
            list: HashMap::new(),
            next_sid: sid,
            handler: Some(handler),
            next_select_index: 0,
        }
    }

    fn get_session(&self, sid: &u32) -> Option<Arc<WebSocketSession>> {
        self.list.get(sid).map(|v| v.to_owned())
    }

    // 随机选择一个session
    fn select_session(&mut self) -> Option<Arc<WebSocketSession>> {
        match self.list.len() {
            0 => None,
            1 => {
                let session = self.list.iter().next().unwrap().1.to_owned();
                if session.is_valid() {
                    Some(session)
                } else {
                    None
                }
            }
            count @ _ => {
                // 多于一个，那么随机选择一个
                for _ in 0..count {
                    let index = self.next_select_index % count;
                    self.next_select_index += 1;

                    let session = self.list.iter().nth(index).unwrap().1.to_owned();
                    if session.is_valid() {
                        return Some(session);
                    }
                }

                // 所有session都无效
                None
            }
        }
    }

    fn new_session(
        &mut self,
        source: String,
        conn_info: (SocketAddr, SocketAddr),
    ) -> BuckyResult<Arc<WebSocketSession>> {
        let sid = self.next_sid;
        self.next_sid += 1;
        if self.next_sid == u32::MAX {
            self.next_sid = 0;
        }

        let handler = self.handler.as_ref().map(|item| item.clone_handler());
        if handler.is_none() {
            let msg = format!(
                "new ws session but request handler is empty! source={}",
                source
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        let session = WebSocketSession::new(
            sid,
            source,
            conn_info,
            handler.unwrap(),
        );

        let session = Arc::new(session);
        if let Some(_) = self.list.insert(sid, session.clone()) {
            unreachable!();
        }

        Ok(session)
    }

    pub fn remove_session(&mut self, sid: u32) -> Option<Arc<WebSocketSession>> {
        self.list.remove(&sid)
    }
}

#[derive(Clone)]
pub struct WebSocketSessionManager(Arc<Mutex<WebSocketSessionManagerInner>>);

impl WebSocketSessionManager {
    pub fn new(handler: Box<dyn WebSocketRequestHandler>) -> Self {
        Self(Arc::new(Mutex::new(WebSocketSessionManagerInner::new(
            handler,
        ))))
    }

    pub fn get_session(&self, sid: &u32) -> Option<Arc<WebSocketSession>> {
        self.0.lock().unwrap().get_session(sid)
    }

    pub fn select_session(&self) -> Option<Arc<WebSocketSession>> {
        self.0.lock().unwrap().select_session()
    }

    pub fn stop(&self) {
        let mut inner = self.0.lock().unwrap();
        assert!(inner.handler.is_some());
        inner.handler = None;
    }

    pub fn new_session(
        &self,
        service_url: &Url,
        conn_info: (SocketAddr, SocketAddr),
    ) -> BuckyResult<Arc<WebSocketSession>> {
        self.0
            .lock()
            .unwrap()
            .new_session(service_url.to_string(), conn_info)
    }

    pub async fn run_client_session<S>(
        &self,
        service_url: &Url,
        session: Arc<WebSocketSession>,
        stream: S,
    ) -> BuckyResult<()>
    where
        S: Read + Write + Unpin + Send + 'static,
    {
        let inner = self.0.clone();

        let ret = WebSocketSession::run_client(session.clone(), &service_url, stream).await;

        let current = inner.lock().unwrap().remove_session(session.sid());
        if current.is_none() {
            unreachable!("session not exists! sid={}", session.sid());
        }

        ret
    }

    pub fn run_server_session<S>(
        &self,
        source: String,
        conn_info: (SocketAddr, SocketAddr),
        stream: S,
    ) where
        S: Read + Write + Unpin + Send + 'static,
    {
        let inner = self.0.clone();
        let session = inner
            .lock()
            .unwrap()
            .new_session(source, conn_info)
            .unwrap();
        async_std::task::spawn(async move {
            let _ = WebSocketSession::run_server(session.clone(), stream).await;

            let ret = inner.lock().unwrap().remove_session(session.sid());
            if ret.is_none() {
                unreachable!("session not exists! sid={}", session.sid());
            }
        });
    }
}
