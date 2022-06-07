use crate::*;

use async_std::sync::{Mutex as AsyncMutex, MutexGuardArc as AsyncMutexGuardArc};
use std::sync::{Arc};

#[derive(Debug, Clone)]
pub(crate) struct PathLockRequest {
    pub path: String,
    pub sid: u64,
    pub expired: u64,
}

struct PathLockMutex {
    lock: Arc<AsyncMutex<u32>>,
    guard: AsyncMutexGuardArc<u32>,
}

impl std::fmt::Debug for PathLockMutex {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Ok(())
    }
}

impl PathLockMutex {
    pub async fn new() -> Self {
        let lock = Arc::new(AsyncMutex::new(0));
        let guard = lock.lock_arc().await;

        Self { lock, guard }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PathUnlockRequest {
    // path为空，那么释放这个session的所有lock
    pub path: Option<String>,
    pub sid: u64,
}

#[derive(Debug)]
struct PathLockData {
    path: String,
    sid: u64,
    expired: u64,
    ref_count: u64,

    mutex: PathLockMutex,
}

struct PathLockList {
    list: Vec<PathLockData>,
}

impl PathLockList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    // 所有lock path以/结束
    fn fix_path(path: &str) -> String {
        let path_segs: Vec<&str> = path.split("/").filter(|&seg| seg.len() > 0).collect();
        let path = path_segs.join("/");
        format!("{}/", path)
    }

    // /a/b/c加锁，那么/a, /a/b, /a/b/c/d都视为锁定
    fn match_lock_mut(&mut self, path: &str) -> Option<(usize, &mut PathLockData)> {
        assert!(path.ends_with("/"));

        for item in &mut self.list.iter_mut().enumerate() {
            if !path.starts_with(&item.1.path) && !item.1.path.starts_with(path) {
                continue;
            }

            return Some(item);
        }

        None
    }

    fn match_lock(&self, path: &str) -> Option<(usize, &PathLockData)> {
        assert!(path.ends_with("/"));

        for item in &mut self.list.iter().enumerate() {
            if !path.starts_with(&item.1.path) && !item.1.path.starts_with(path) {
                continue;
            }

            return Some(item);
        }

        None
    }

    fn find_lock(&self, path: &str, sid: u64) -> Option<(usize, &PathLockData)> {
        for item in &mut self.list.iter().enumerate() {
            if path == &item.1.path && sid == item.1.sid {
                return Some(item);
            }
        }

        None
    }

    // check if a path had been locked
    pub fn try_enter_path(&self, path: &str, sid: u64) -> BuckyResult<()> {
        let path = Self::fix_path(path);
        match self.match_lock(&path) {
            Some((_, data)) => {
                if data.sid != sid {
                    let msg = format!("path had been locked already! require=({},{}), current=({},{})", path, sid, data.path, data.sid);
                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::WouldBlock, msg))
                } else {
                    Ok(())
                }
            }
            None => Ok(())
        }
    }

    // 检查一组锁是否仍然有效
    pub fn check_lock_valid(&self, req_list: Vec<PathLockRequest>) -> BuckyResult<()> {
        for req in req_list {
            let path = Self::fix_path(&req.path);
            let ret = self.find_lock(&path, req.sid);
            if ret.is_none() {
                let msg = format!("path lock not found! req={:?}", req);
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        Ok(())
    }

    pub async fn try_lock(
        &mut self,
        req: &PathLockRequest,
        mutex: Option<PathLockMutex>,
    ) -> Result<(), Arc<AsyncMutex<u32>>> {
        self.remove_expired();

        let path = Self::fix_path(&req.path);
        let ret = self.match_lock_mut(&path);
        match ret {
            Some((_index, current_lock)) => {
                // 命中了当前的lock
                // 检查sid是否匹配
                if current_lock.sid != req.sid {
                    return Err(current_lock.mutex.lock.clone());
                }

                if path == current_lock.path {
                    current_lock.ref_count += 1;
                    if req.expired > current_lock.expired {
                        current_lock.expired = req.expired;
                    }

                    info!("ref lock: {:?}", current_lock);
                    return Ok(());
                }
            }
            None => {
                // drop(current);
            }
        };

        let mutex = match mutex {
            Some(v) => v,
            None => PathLockMutex::new().await,
        };

        // 增加一个新lock
        let data = PathLockData {
            path,
            sid: req.sid,
            ref_count: 1,
            expired: req.expired,

            mutex,
        };

        info!("new lock: {:?}", data);

        self.list.push(data);
        Ok(())
    }

    pub async fn lock_list(&mut self, req_list: Vec<PathLockRequest>) -> BuckyResult<()> {
        assert!(req_list.len() > 0);

        self.remove_expired();

        // 首先检查是否所有锁是否都可以成功
        let mut all = vec![];
        for mut req in req_list {
            req.path = Self::fix_path(&req.path);
            let ret = self.match_lock_mut(&req.path);
            match ret {
                Some((index, current_lock)) => {
                    // 命中了当前的lock
                    // 检查sid是否匹配
                    if current_lock.sid != req.sid {
                        let msg = format!(
                            "lock already taken by other session! current=({}, {}), require=({}, {})",
                            current_lock.path, current_lock.sid, req.path, req.sid
                        );
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
                    }

                    if current_lock.path == req.path {
                        all.push((req, Some(index)));
                    } else {
                        all.push((req, None));
                    }
                }
                None => {
                    all.push((req, None));
                }
            }
        }

        for (req, index) in all {
            match index {
                Some(index) => {
                    let current_lock = &mut self.list[index];
                    current_lock.ref_count += 1;
                    if req.expired > current_lock.expired {
                        current_lock.expired = req.expired;
                    }
                    info!("ref lock: {:?}", current_lock);
                }
                None => {
                    // 增加一个新lock

                    let data = PathLockData {
                        path: req.path,
                        sid: req.sid,
                        ref_count: 1,
                        expired: req.expired,

                        mutex: PathLockMutex::new().await,
                    };

                    info!("new lock: {:?}", data);
                    self.list.push(data);
                }
            }
        }

        Ok(())
    }

    pub fn unlock(&mut self, req: PathUnlockRequest) -> BuckyResult<()> {
        self.remove_expired();

        match req.path {
            Some(path) => self.unlock_by_path(path, req.sid),
            None => self.unlock_by_sid(req.sid),
        }
    }

    fn unlock_by_sid(&mut self, sid: u64) -> BuckyResult<()> {
        let mut removed_list = vec![];
        for (index, item) in &mut self.list.iter().enumerate() {
            assert!(item.ref_count > 0);
            if item.sid != sid {
                continue;
            }

            // 按session移除，不考虑引用计数
            removed_list.push(index);
        }

        for index in removed_list.into_iter().rev() {
            let lock_item = &self.list[index];
            assert_eq!(lock_item.sid, sid);
            info!(
                "path unlocked by session: path={}, sid={}",
                lock_item.path, lock_item.sid
            );
            drop(lock_item);
            self.list.remove(index);
        }
        Ok(())
    }

    fn unlock_by_path(&mut self, path: String, sid: u64) -> BuckyResult<()> {
        let path = Self::fix_path(&path);
        for (index, item) in &mut self.list.iter_mut().enumerate() {
            if path != item.path {
                continue;
            }

            assert!(item.ref_count > 0);
            if item.sid != sid {
                let msg = format!(
                    "path unlock but sid unmatch! path={}, current={}, require={}",
                    path, item.sid, sid
                );
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }

            item.ref_count -= 1;
            if item.ref_count == 0 {
                info!("path unlocked: path={}, sid={}", path, sid);
                self.list.remove(index);
            }

            return Ok(());
        }

        let msg = format!("path unlock but not found! path={}, sid={}", path, sid);
        warn!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
    }

    pub fn remove_expired(&mut self) {
        let now = bucky_time_now();
        let mut removed_list = vec![];
        for (index, item) in &mut self.list.iter().enumerate() {
            assert!(item.ref_count > 0);
            if item.expired > 0 && now >= item.expired {
                // 过期了需要移除，不考虑引用计数
                removed_list.push(index);
            }
        }

        for index in removed_list.into_iter().rev() {
            let lock_item = &self.list[index];
            assert!(lock_item.expired != 0 && now >= lock_item.expired);
            info!(
                "path unlocked on expired: path={}, sid={}, expired={}, now={}",
                lock_item.path, lock_item.sid, lock_item.expired, now,
            );
            drop(lock_item);
            self.list.remove(index);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ObjectMapPathLock {
    lock_list: Arc<AsyncMutex<PathLockList>>,
}

impl ObjectMapPathLock {
    pub fn new() -> Self {
        Self {
            lock_list: Arc::new(AsyncMutex::new(PathLockList::new())),
        }
    }

    pub async fn try_enter_path(&self, full_path: &str, sid: u64) -> BuckyResult<()> {
        self.lock_list.lock().await.try_enter_path(full_path, sid)
    }

    pub async fn try_enter_path_and_key(&self, path: &str, key: &str, sid: u64) -> BuckyResult<()> {
        let full_path = format!("{}/{}", path.trim_end_matches("/"), key);
        self.lock_list.lock().await.try_enter_path(&full_path, sid)
    }

    pub async fn check_lock_valid(&self, req_list: Vec<PathLockRequest>) -> BuckyResult<()> {
        self.lock_list.lock().await.check_lock_valid(req_list)
    }

    pub async fn try_lock(&self, req: PathLockRequest) -> BuckyResult<()> {
        self.lock_list
            .lock()
            .await
            .try_lock(&req, None)
            .await
            .map_err(|_| {
                let msg = format!(
                    "lock already taken by other session! current=({}, {}), require=({}, {})",
                    req.path, req.sid, req.path, req.sid
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::AlreadyExists, msg)
            })
    }

    pub async fn lock(&self, req: PathLockRequest) {
        let mut mutex = None;
        loop {
            let ret = self.lock_list.lock().await.try_lock(&req, mutex).await;
            match ret {
                Ok(()) => break,
                Err(lock) => {
                    let guard = lock.lock_arc().await;
                    mutex = Some(PathLockMutex { lock, guard });
                }
            }
        }
    }

    pub async fn lock_list(&self, mut req_list: Vec<PathLockRequest>) {
        req_list.sort_by(|a, b| {
            a.path.partial_cmp(&b.path).unwrap()
        });

        for req in req_list {
            self.lock(req).await;
        }
    }

    pub async fn try_lock_list(&self, mut req_list: Vec<PathLockRequest>) -> BuckyResult<()> {
        req_list.sort_by(|a, b| {
            a.path.partial_cmp(&b.path).unwrap()
        });

        self.lock_list.lock().await.lock_list(req_list).await
    }

    pub async fn unlock(&self, req: PathUnlockRequest) -> BuckyResult<()> {
        self.lock_list.lock().await.unlock(req)
    }
}

#[cfg(test)]
mod test_lock {
    use super::*;

    async fn test_waiting_lock(lock: &ObjectMapPathLock) {
        info!("test_waiting_lock...");

        let req = PathLockRequest {
            path: "/x/b/c".to_owned(),
            expired: 0,
            sid: 1,
        };

        lock.lock(req).await;
        info!("lock /x/b/c");

        // 重复lock，成功
        let req = PathLockRequest {
            path: "/x/b/c".to_owned(),
            expired: 0,
            sid: 1,
        };

        lock.lock(req).await;
        info!("lock /x/b/c");

        lock.try_enter_path("/x1", 100).await.unwrap();
        lock.try_enter_path("/x/b1", 100).await.unwrap();

        lock.try_enter_path("/x", 100).await.unwrap_err();
        lock.try_enter_path("/x/b", 100).await.unwrap_err();
        lock.try_enter_path("/x/b/c", 100).await.unwrap_err();
        lock.try_enter_path("/x/b/c/d", 100).await.unwrap_err();

        // 不同的sid覆盖加锁，等待

        let lock1 = lock.clone();
        async_std::task::spawn(async move {
            let req = PathLockRequest {
                path: "/x/b/c/d".to_owned(),
                expired: 0,
                sid: 2,
            };

            info!("will acquire /x/b/c/d sid=2");
            lock1.lock(req).await;
            info!("end acquire /x/b/c/d sid=2");

            async_std::task::sleep(std::time::Duration::from_secs(2)).await;
            let req = PathUnlockRequest { path: None, sid: 2 };
            lock1.unlock(req).await.unwrap();
            info!("end release /x/b/c/d, sid=2");
        });

        let lock1 = lock.clone();
        async_std::task::spawn(async move {
            let req = PathLockRequest {
                path: "/x/b/c/d".to_owned(),
                expired: 0,
                sid: 3,
            };

            info!("will acquire /x/b/c/d, sid=3");
            lock1.lock(req).await;
            info!("end acquire /x/b/c/d, sid=3");

            async_std::task::sleep(std::time::Duration::from_secs(2)).await;
            let req = PathUnlockRequest { path: None, sid: 3 };
            lock1.unlock(req).await.unwrap();
            info!("end release /x/b/c/d, sid=3");
        });

        let req = PathUnlockRequest { path: None, sid: 1 };

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        info!("will release sid lock 1");
        lock.unlock(req).await.unwrap();
        info!("end release sid lock 1");

        async_std::task::sleep(std::time::Duration::from_secs(5)).await;
    }

    async fn test_try_lock1(lock: &ObjectMapPathLock) {
        let req = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 1,
        };

        lock.try_lock(req).await.unwrap();

        // 重复lock，成功
        let req = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 1,
        };

        lock.try_lock(req).await.unwrap();

        // 不同的sid覆盖加锁，失败
        let req = PathLockRequest {
            path: "/a/b/c/d".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock(req).await.unwrap_err();

        let req = PathLockRequest {
            path: "/a/b/".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock(req).await.unwrap_err();

        // 不同sid+不同path加锁，成功
        let req = PathLockRequest {
            path: "/a/b/d".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock(req).await.unwrap();

        // 批量lock，部分失败则失败
        let req = PathLockRequest {
            path: "/a/b/".to_owned(),
            expired: 0,
            sid: 2,
        };
        let req2 = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock_list(vec![req, req2]).await.unwrap_err();

        // 单独解锁
        let req = PathUnlockRequest {
            path: Some("/a/b/c".to_owned()),
            sid: 2,
        };
        lock.unlock(req).await.unwrap_err();

        // 第一次成功解锁
        let req = PathUnlockRequest {
            path: Some("/a/b/c".to_owned()),
            sid: 1,
        };
        lock.unlock(req).await.unwrap();

        // sid=1持有该锁并且引用计数不为0，加锁失败
        let req = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock(req).await.unwrap_err();

        // 基于sid整体解锁
        let req = PathUnlockRequest { path: None, sid: 1 };
        lock.unlock(req).await.unwrap();

        // sid=2成功获取该锁
        let req = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 2,
        };

        lock.try_lock(req.clone()).await.unwrap();

        lock.check_lock_valid(vec![req]).await.unwrap();

        // invalid check
        let req = PathLockRequest {
            path: "/a/b/c".to_owned(),
            expired: 0,
            sid: 3,
        };

        lock.check_lock_valid(vec![req]).await.unwrap_err();
    }

    async fn test_lock() {
        let lock = ObjectMapPathLock::new();

        test_try_lock1(&lock).await;
        test_waiting_lock(&lock).await;
    }

    #[test]
    fn test() {
        crate::init_simple_log("test-object-map-lock", Some("debug"));
        async_std::task::block_on(async move {
            test_lock().await;
        });
    }
}
