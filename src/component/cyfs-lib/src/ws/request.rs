use super::packet::*;
use super::session::*;
use cyfs_base::{bucky_time_now, BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_debug::Mutex;

use async_trait::async_trait;
use futures::future::{AbortHandle, Abortable};
use futures::prelude::*;
use lru_time_cache::LruCache;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
use std::time::Duration;

// ws request的默认超时时间
const WS_REQUEST_DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 10 * 10);

#[async_trait]
pub trait WebSocketRequestHandler: Send + Sync + 'static {
    async fn on_request(
        &self,
        requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: Vec<u8>,
    ) -> BuckyResult<Option<Vec<u8>>> {
        self.process_string_request(requestor, cmd, content).await
    }

    async fn process_string_request(
        &self,
        requestor: Arc<WebSocketRequestManager>,
        cmd: u16,
        content: Vec<u8>,
    ) -> BuckyResult<Option<Vec<u8>>> {
        let content = String::from_utf8(content).map_err(|e| {
            let msg = format!(
                "decode ws packet as string failed! sid={}, cmd={}, {}",
                requestor.sid(),
                cmd,
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.on_string_request(requestor, cmd, content)
            .await
            .map(|v| v.map(|v| v.into_bytes()))
    }

    async fn on_string_request(
        &self,
        _requestor: Arc<WebSocketRequestManager>,
        _cmd: u16,
        _content: String,
    ) -> BuckyResult<Option<String>> {
        unimplemented!();
    }

    async fn on_session_begin(&self, session: &Arc<WebSocketSession>);
    async fn on_session_end(&self, session: &Arc<WebSocketSession>);

    fn clone_handler(&self) -> Box<dyn WebSocketRequestHandler>;
}

struct RequestItem {
    seq: u16,
    send_tick: u64,
    resp: Option<BuckyResult<Vec<u8>>>,
    waker: Option<AbortHandle>,
}

impl RequestItem {
    fn new(seq: u16) -> Self {
        Self {
            seq,
            send_tick: bucky_time_now(),
            resp: None,
            waker: None,
        }
    }

    fn resp(&mut self, code: BuckyErrorCode) {
        if let Some(waker) = self.waker.take() {
            if self.resp.is_none() {
                self.resp = Some(Err(BuckyError::from(code)));
            } else {
                warn!(
                    "end ws request with {:?} but already has resp! send_tick={}, seq={}",
                    code, self.send_tick, self.seq
                );
                unreachable!();
            }

            waker.abort();
        }
    }

    fn timeout(&mut self) {
        self.resp(BuckyErrorCode::Timeout);
    }

    fn abort(&mut self) {
        self.resp(BuckyErrorCode::Aborted);
    }
}

impl Drop for RequestItem {
    fn drop(&mut self) {
        // info!("will drop ws request! seq={}", self.seq);
        self.abort();
    }
}

struct WebSocketRequestContainer {
    list: LruCache<u16, Arc<Mutex<RequestItem>>>,
    next_seq: u16,
}

impl WebSocketRequestContainer {
    fn new() -> Self {
        let list = LruCache::with_expiry_duration(WS_REQUEST_DEFAULT_TIMEOUT);

        Self { list, next_seq: 1 }
    }

    fn new_request(
        &mut self,
        sid: u32,
    ) -> (
        u16,
        Arc<Mutex<RequestItem>>,
        Vec<(u16, Arc<Mutex<RequestItem>>)>,
    ) {
        let seq = self.next_seq;
        self.next_seq += 1;
        if self.next_seq == u16::MAX {
            warn!("ws request seq roll back! sid={}", sid);
            self.next_seq = 1;
        }

        let req_item = RequestItem::new(seq);

        let req_item = Arc::new(Mutex::new(req_item));
        let (old, mut list) = self.list.notify_insert(seq, req_item.clone());

        if let Some(old) = old {
            // 正常情况下不应该到这里，除非短时间内发了超大量的request，导致seq回环
            let seq;
            {
                let old_item = old.lock().unwrap();
                error!(
                    "replace old with same seq! sid={}, seq={}, send_tick={}",
                    sid, old_item.seq, old_item.send_tick
                );
                seq = old_item.seq;
            }

            // FIXME 先用超时对待
            list.push((seq, old));
        }

        (seq, req_item, list)
    }

    /*
    fn bind_waker(&mut self, seq: u16, waker: AbortHandle) {
        let (item, list) = self.list.notify_get_mut(&seq);
        if let Some(item) = item {
            let mut item = item.lock().unwrap();
            assert!(item.waker.is_none());
            item.waker = Some(waker);
        } else {
            unreachable!();
        }
        if !list.is_empty() {
            self.on_timeout(list);
        }
    }
    */

    fn remove_request(&mut self, seq: u16) -> Option<Arc<Mutex<RequestItem>>> {
        assert!(seq > 0);

        self.list.remove(&seq)
    }

    fn check_timeout(&mut self) -> Vec<(u16, Arc<Mutex<RequestItem>>)> {
        // 直接清除过期的元素，不能迭代这些元素，否则会导致这些元素被更新时间戳
        let (_, list) = self.list.notify_get(&0);

        list
    }

    // 清空所有元素
    fn clear(&mut self) {
        for (seq, item) in self.list.iter() {
            info!("will abort ws request: seq={}", seq);
            item.lock().unwrap().abort();
        }

        self.list.clear();
    }

    fn on_timeout(sid: u32, list: Vec<(u16, Arc<Mutex<RequestItem>>)>) {
        for (seq, item) in list {
            warn!("ws request droped on timeout! sid={}, seq={}", sid, seq);

            let mut item = item.lock().unwrap();
            if item.waker.is_some() {
                item.timeout();
            } else {
                // timeout的同时收到了应答，发生了竞争
                warn!(
                    "ws request timeout but already waked! sid={}, seq={}",
                    sid, seq
                );
            }
        }
    }
}

pub struct WebSocketRequestManager {
    reqs: Arc<Mutex<WebSocketRequestContainer>>,
    session: Arc<Mutex<Option<Arc<WebSocketSession>>>>,
    sid: AtomicU32,
    monitor_canceler: Arc<Mutex<Option<AbortHandle>>>,
    handler: Box<dyn WebSocketRequestHandler>,
}

impl Drop for WebSocketRequestManager {
    fn drop(&mut self) {
        let mut monitor_canceler = self.monitor_canceler.lock().unwrap();
        if let Some(canceler) = monitor_canceler.take() {
            info!("will stop ws request monitor: sid={}", self.sid());
            canceler.abort();
        }

        self.reqs.lock().unwrap().clear();
    }
}

impl WebSocketRequestManager {
    pub fn new(handler: Box<dyn WebSocketRequestHandler>) -> Self {
        let reqs = WebSocketRequestContainer::new();

        Self {
            reqs: Arc::new(Mutex::new(reqs)),
            session: Arc::new(Mutex::new(None)),
            sid: AtomicU32::new(0),
            monitor_canceler: Arc::new(Mutex::new(None)),
            handler,
        }
    }

    pub fn sid(&self) -> u32 {
        self.sid.load(Ordering::Relaxed)
    }

    pub fn session(&self) -> Option<Arc<WebSocketSession>> {
        self.session.lock().unwrap().clone()
    }

    pub fn is_session_valid(&self) -> bool {
        self.session.lock().unwrap().is_some()
    }

    pub fn bind_session(&self, session: Arc<WebSocketSession>) {
        {
            let mut local = self.session.lock().unwrap();
            assert!(local.is_none());

            self.sid.store(session.sid(), Ordering::SeqCst);
            *local = Some(session);
        }

        self.monitor();
    }

    pub fn unbind_session(&self) {
        self.stop_monitor();

        // 强制所有pending的请求为超时
        self.reqs.lock().unwrap().clear();

        let _ = {
            let mut local = self.session.lock().unwrap();
            assert!(local.is_some());

            debug!(
                "ws request manager unbind session! sid={}",
                local.as_ref().unwrap().sid()
            );
            local.take()
        };
    }

    // 收到了msg
    pub async fn on_msg(
        requestor: Arc<WebSocketRequestManager>,
        packet: WSPacket,
    ) -> BuckyResult<()> {
        let cmd = packet.header.cmd;
        if cmd > 0 {
            let seq = packet.header.seq;

            let resp = requestor
                .handler
                .on_request(requestor.clone(), cmd, packet.content)
                .await?;

            // 如果seq==0，表示不需要应答，那么应该返回None
            if resp.is_none() {
                assert!(seq == 0);
            } else {
                assert!(seq > 0);

                // 发起应答,cmd需要设置为0
                let resp_packet = WSPacket::new_from_bytes(seq, 0, resp.unwrap());
                let buf = resp_packet.encode();
                requestor.post_to_session(buf).await?;
            }
        } else {
            requestor.on_resp(packet).await?;
        }
        Ok(())
    }

    // 发送一个字符串请求
    pub async fn post_req(&self, cmd: u16, msg: String) -> BuckyResult<String> {
        let content = self.post_bytes_req(cmd, msg.into_bytes()).await?;

        match String::from_utf8(content) {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = format!(
                    "decode ws resp as string failed! sid={}, cmd={}, {}",
                    self.sid(),
                    cmd,
                    e
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    // 发送一个请求并等待应答
    pub async fn post_bytes_req(&self, cmd: u16, msg: Vec<u8>) -> BuckyResult<Vec<u8>> {
        let (seq, item, timeout_list) = self.reqs.lock().unwrap().new_request(self.sid());
        assert!(seq > 0);

        // 首先处理超时的
        if !timeout_list.is_empty() {
            WebSocketRequestContainer::on_timeout(self.sid(), timeout_list);
        }

        // Init waker before send the packet
        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        {
            let mut item = item.lock().unwrap();
            assert!(item.waker.is_none());
            item.waker = Some(abort_handle);
        }

        let packet = WSPacket::new_from_bytes(seq, cmd, msg);
        let buf = packet.encode();
        if let Err(e) = self.post_to_session(buf).await {
            self.reqs.lock().unwrap().remove_request(seq);

            return Err(e);
        }

        // info!("request send complete, now will wait for resp! cmd={}", cmd);

        // 等待唤醒
        let future = Abortable::new(async_std::future::pending::<()>(), abort_registration);
        future.await.unwrap_err();

        // 应答
        let mut item = item.lock().unwrap();
        if let Some(resp) = item.resp.take() {
            resp
        } else {
            unreachable!(
                "ws request item waked up without resp: sid={}, seq={}",
                self.sid(),
                item.seq
            );
        }
    }

    // 不带应答的请求
    async fn post_req_without_resp(&self, cmd: u16, msg: String) -> BuckyResult<()> {
        self.post_bytes_req_without_resp(cmd, msg.into_bytes())
            .await
    }

    async fn post_bytes_req_without_resp(&self, cmd: u16, msg: Vec<u8>) -> BuckyResult<()> {
        let packet = WSPacket::new_from_bytes(0, cmd, msg);
        let buf = packet.encode();

        self.post_to_session(buf).await
    }

    // 收到了应答
    async fn on_resp(&self, packet: WSPacket) -> BuckyResult<()> {
        assert!(packet.header.cmd == 0);
        assert!(packet.header.seq > 0);

        let seq = packet.header.seq;
        let ret = self.reqs.lock().unwrap().remove_request(seq);
        if ret.is_none() {
            let msg = format!(
                "ws request recv resp but already been removed! sid={}, seq={}",
                self.sid(),
                seq
            );

            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let item = ret.unwrap();

        // 保存应答并唤醒
        let mut item = item.lock().unwrap();
        if let Some(waker) = item.waker.take() {
            if item.resp.is_none() {
                item.resp = Some(Ok(packet.content));
            } else {
                warn!(
                    "ws request recv resp but already has local resp! sid={}, seq={}",
                    self.sid(),
                    seq
                );
                unreachable!();
            }

            drop(item);

            waker.abort();
        } else {
            warn!(
                "ws request recv resp but already timeout! sid={}, seq={}",
                self.sid(),
                seq
            );
        }

        Ok(())
    }

    async fn post_to_session(&self, msg: Vec<u8>) -> BuckyResult<()> {
        let ret = self.session.lock().unwrap().clone();
        if ret.is_none() {
            warn!("ws session not exists: {}", self.sid());
            return Err(BuckyError::from(BuckyErrorCode::NotConnected));
        }

        let session = ret.unwrap();
        session.post_msg(msg).await.map_err(|e| e)?;
        Ok(())
    }

    fn monitor(&self) {
        let reqs = self.reqs.clone();
        let sid = self.sid();

        let (fut, handle) = future::abortable(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(15));
            while let Some(_) = interval.next().await {
                let list = reqs.lock().unwrap().check_timeout();

                if !list.is_empty() {
                    WebSocketRequestContainer::on_timeout(sid, list);
                }
            }
        });

        // 保存canceler，用以session结束时候取消
        let mut monitor_canceler = self.monitor_canceler.lock().unwrap();
        assert!(monitor_canceler.is_none());
        *monitor_canceler = Some(handle);

        async_std::task::spawn(async move {
            match fut.await {
                Ok(_) => {
                    info!("ws request monitor complete, sid={}", sid);
                    // 不应该到这里，只有被abort一种可能
                    unreachable!();
                }
                Err(_aborted) => {
                    info!("ws request monitor breaked, sid={}", sid);
                }
            };
        });
    }

    fn stop_monitor(&self) {
        let mut monitor_canceler = self.monitor_canceler.lock().unwrap();
        if let Some(canceler) = monitor_canceler.take() {
            debug!("will stop ws request monitor: sid={}", self.sid());
            canceler.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::future::{AbortHandle, Abortable};

    async fn test_wakeup() {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        abort_handle.abort();

        async_std::task::spawn(async move {
            async_std::task::sleep(std::time::Duration::from_secs(2)).await;
            abort_handle.abort();
        });

        // 等待唤醒
        let future = Abortable::new(async_std::future::pending::<()>(), abort_registration);
        future.await.unwrap_err();

        println!("future wait complete!");

        async_std::task::sleep(std::time::Duration::from_secs(3)).await;
    }

    #[test]
    fn test() {
        async_std::task::block_on(async move {
            test_wakeup().await;
        })
    }
}
