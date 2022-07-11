use super::cache::*;
use super::lock::*;
use super::path_env::*;
use super::single_env::*;
use crate::*;

use async_std::sync::Mutex as AsyncMutex;
use std::future::Future;
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, RwLock};

#[async_trait::async_trait]
pub trait ObjectMapRootEvent: Sync + Send + 'static {
    async fn root_updated(
        &self,
        dec_id: &Option<ObjectId>,
        new_root_id: ObjectId,
        prev_id: ObjectId,
    ) -> BuckyResult<()>;
}

pub type ObjectMapRootEventRef = Arc<Box<dyn ObjectMapRootEvent>>;

#[derive(Clone)]
pub struct ObjectMapRootHolder {
    dec_id: Option<ObjectId>,

    // 当前的读写锁，只有在持有update_lock情况下，才可以更新
    root: Arc<RwLock<ObjectId>>,
    update_lock: Arc<AsyncMutex<()>>,
    event: ObjectMapRootEventRef,
}

impl ObjectMapRootHolder {
    pub fn new(dec_id: Option<ObjectId>, root: ObjectId, event: ObjectMapRootEventRef) -> Self {
        Self {
            dec_id,
            root: Arc::new(RwLock::new(root)),
            update_lock: Arc::new(AsyncMutex::new(())),
            event,
        }
    }

    pub fn get_current_root(&self) -> ObjectId {
        self.root.read().unwrap().clone()
    }

    // direct set the root_state without notify event
    pub async fn direct_reload_root(&self, new_root_id: ObjectId) {
        let _update_lock = self.update_lock.lock().await;
        let mut current = self.root.write().unwrap();

        info!(
            "reload objectmap root holder's root! dec={:?}, current={}, new={}",
            self.dec_id, *current, new_root_id
        );
        *current = new_root_id;
    }

    // 尝试更新root，同一个root同一时刻只能有一个操作在进行，通过异步锁来保证
    pub async fn update_root<F, Fut>(&self, update_root_fn: F) -> BuckyResult<ObjectId>
    where
        F: FnOnce(ObjectId) -> Fut,
        Fut: Future<Output = BuckyResult<ObjectId>>,
    {
        let _update_lock = self.update_lock.lock().await;
        let root = self.get_current_root();
        let new_root = update_root_fn(root.clone()).await?;
        if new_root != root {
            info!("will update root holder: {} -> {}", root, new_root);

            // 必须先触发事件，通知上层更新全局状态
            if let Err(e) = self
                .event
                .root_updated(&self.dec_id, new_root.clone(), root.clone())
                .await
            {
                error!(
                    "root update event notify error! {} -> {}, {}",
                    root, new_root, e
                );

                return Err(e);
            }

            // 触发事件成功后，才可以更新root-holder
            // 避免这两个操作之间，新的root-holder被使用但全局根状态由于没更新导致的各种异常
            {
                let mut current = self.root.write().unwrap();
                assert_eq!(*current, root);
                *current = new_root.clone();
            }

            info!("root updated! {} -> {}", root, new_root);
        }

        Ok(new_root)
    }
}

#[derive(Clone, Debug, Copy, Eq, PartialEq)]
pub enum ObjectMapOpEnvType {
    Path,
    Single,
}

impl ToString for ObjectMapOpEnvType {
    fn to_string(&self) -> String {
        match *self {
            Self::Path => "path",
            Self::Single => "single",
        }
        .to_owned()
    }
}

impl FromStr for ObjectMapOpEnvType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "path" => Self::Path,
            "single" => Self::Single,

            v @ _ => {
                let msg = format!("unknown op env type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, Copy)]
pub struct OpEnvSessionIDHelper;

// 最高位两位表示op_env的类型
const OP_ENV_PATH_FLAGS: u8 = 0b_00000000;
const OP_ENV_SINGLE_FLAGS: u8 = 0b_00000001;

impl OpEnvSessionIDHelper {
    pub fn get_flags(sid: u64) -> u8 {
        (sid >> 62) as u8
    }

    pub fn get_type(sid: u64) -> BuckyResult<ObjectMapOpEnvType> {
        let flags = Self::get_flags(sid);
        if flags == OP_ENV_PATH_FLAGS {
            Ok(ObjectMapOpEnvType::Path)
        } else if flags == OP_ENV_SINGLE_FLAGS {
            Ok(ObjectMapOpEnvType::Single)
        } else {
            let msg = format!("unknown op_ev sid flags: sid={}, flags={}", sid, flags);
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        }
    }

    pub fn set_type(sid: u64, op_env_type: ObjectMapOpEnvType) -> u64 {
        let flags = match op_env_type {
            ObjectMapOpEnvType::Path => OP_ENV_PATH_FLAGS,
            ObjectMapOpEnvType::Single => OP_ENV_SINGLE_FLAGS,
        };

        //assert!(Self::get_flags(sid) == 0);
        //println!("prev clear: {:#x}", sid);
        let sid = sid & 0b_00111111_11111111_11111111_11111111_11111111_11111111_11111111_11111111;
        //println!("after clear: {:#x}", sid);

        let sid = sid | ((flags as u64) << 62);
        //println!("after set: {:#x}", sid);

        sid
    }
}

#[cfg(test)]
mod test_sid {
    use super::OpEnvSessionIDHelper;
    use crate::*;

    #[test]
    fn test_sid() {
        let sid = 123;
        let t = OpEnvSessionIDHelper::get_type(sid).unwrap();
        assert_eq!(t, ObjectMapOpEnvType::Path);
        let sid = OpEnvSessionIDHelper::set_type(sid, ObjectMapOpEnvType::Single);
        let t = OpEnvSessionIDHelper::get_type(sid).unwrap();
        assert_eq!(t, ObjectMapOpEnvType::Single);

        let sid = OpEnvSessionIDHelper::set_type(sid, ObjectMapOpEnvType::Path);
        let t = OpEnvSessionIDHelper::get_type(sid).unwrap();
        assert_eq!(t, ObjectMapOpEnvType::Path);
    }
}

#[derive(Clone)]
pub enum ObjectMapOpEnv {
    Path(ObjectMapPathOpEnvRef),
    Single(ObjectMapSingleOpEnvRef),
}

impl ObjectMapOpEnv {
    pub fn sid(&self) -> u64 {
        match self {
            Self::Path(value) => value.sid(),
            Self::Single(value) => value.sid(),
        }
    }

    pub fn path_op_env(&self, sid: u64) -> BuckyResult<ObjectMapPathOpEnvRef> {
        match self {
            Self::Path(value) => Ok(value.clone()),
            _ => {
                let msg = format!(
                    "unmatch env type, path_op_env expected, got single_op_env! sid={}",
                    sid
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
            }
        }
    }

    pub fn single_op_env(&self, sid: u64) -> BuckyResult<ObjectMapSingleOpEnvRef> {
        match self {
            Self::Single(value) => Ok(value.clone()),
            _ => {
                let msg = format!(
                    "unmatch env type, single_op_env expected, got path_op_env! sid={}",
                    sid
                );
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
            }
        }
    }

    pub async fn get_current_root(&self) -> BuckyResult<ObjectId> {
        match self {
            ObjectMapOpEnv::Path(env) => Ok(env.root()),
            ObjectMapOpEnv::Single(env) => match env.get_current_root().await {
                Some(root) => Ok(root),
                None => {
                    let msg = format!("single op_env root not been init yet! sid={}", env.sid());
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, msg))
                }
            },
        }
    }

    pub async fn update(&self) -> BuckyResult<ObjectId> {
        match self {
            ObjectMapOpEnv::Path(env) => env.update().await,
            ObjectMapOpEnv::Single(env) => env.update().await,
        }
    }

    pub async fn commit(self) -> BuckyResult<ObjectId> {
        match self {
            ObjectMapOpEnv::Path(env) => env.commit().await,
            ObjectMapOpEnv::Single(env) => env.commit().await,
        }
    }

    pub fn abort(self) -> BuckyResult<()> {
        match self {
            ObjectMapOpEnv::Path(env) => env.abort(),
            ObjectMapOpEnv::Single(env) => env.abort(),
        }
    }

    pub fn is_dropable(&self) -> bool {
        match self {
            ObjectMapOpEnv::Path(env) => env.is_dropable(),
            ObjectMapOpEnv::Single(env) => env.is_dropable(),
        }
    }
}


use std::collections::HashMap;

#[derive(Clone)]
pub struct ObjectMapOpEnvContainer {
    all: Arc<Mutex<HashMap<u64, ObjectMapOpEnv>>>,
}

impl ObjectMapOpEnvContainer {
    pub(crate) fn new() -> Self {
        let ret = Self {
            all: Arc::new(Mutex::new(HashMap::new())),
        };

        // 自动启动定期gc
        ret.start_monitor();

        ret
    }

    pub fn start_monitor(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(30)).await;
                this.gc_once();
            }
        });
    }

    fn gc_once(&self) {
        let mut expired_list = vec![];
        self.all.lock().unwrap().retain(|sid, op_env| {
            if op_env.is_dropable() {
                expired_list.push((*sid, op_env.clone()));
                false
            } else {
                true
            }
        });

        self.gc_list(expired_list);
    }

    // 回收超时的op_env列表
    fn gc_list(&self, expired_list: Vec<(u64, ObjectMapOpEnv)>) {
        for (sid, op_env) in expired_list {
            warn!("will gc managed op_env on drop: sid={}", sid);
            if let Err(e) = op_env.abort() {
                error!("op_env abort error! sid={}, {}", sid, e);
            }
        }
    }

    pub fn add_env(&self, env: ObjectMapOpEnv) {
        let sid = env.sid();
        let prev = self.all.lock().unwrap().insert(sid, env);
        assert!(prev.is_none());
    }

    pub fn get_op_env(&self, sid: u64) -> BuckyResult<ObjectMapOpEnv> {
        let ret = {
            let ret = self.all.lock().unwrap().get(&sid).cloned();
            let result = match ret {
                Some(value) => Ok(value),
                None => {
                    let msg = format!("op_env not found! sid={}", sid);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            };

            result
        };

        ret
    }

    pub fn get_path_op_env(&self, sid: u64) -> BuckyResult<ObjectMapPathOpEnvRef> {
        let op_env = self.get_op_env(sid)?;
        op_env.path_op_env(sid)
    }

    pub fn get_single_op_env(&self, sid: u64) -> BuckyResult<ObjectMapSingleOpEnvRef> {
        let op_env = self.get_op_env(sid)?;
        op_env.single_op_env(sid)
    }

    pub async fn get_current_root(&self, sid: u64) -> BuckyResult<ObjectId> {
        let op_env = self.get_op_env(sid)?;

        op_env.get_current_root().await
    }

    pub async fn update(&self, sid: u64) -> BuckyResult<ObjectId> {
        let op_env = self.get_op_env(sid)?;

        op_env.update().await
    }

    pub async fn commit(&self, sid: u64) -> BuckyResult<ObjectId> {
        let ret = self.all.lock().unwrap().remove(&sid);
        if ret.is_none() {
            let msg = format!("op_env not found! sid={}", sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        ret.unwrap().commit().await
    }

    pub fn abort(&self, sid: u64) -> BuckyResult<()> {
        let ret = self.all.lock().unwrap().remove(&sid);
        if ret.is_none() {
            let msg = format!("op_env not found! sid={}", sid);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        ret.unwrap().abort()
    }
}

// 用来管理root的管理器
pub struct ObjectMapRootManager {
    // ObjectMap的核心属性
    owner: Option<ObjectId>,
    dec_id: Option<ObjectId>,

    // 为每个op_env分配唯一的sid
    next_sid: AtomicU64,

    // 所属的root
    root: ObjectMapRootHolder,

    // 一个root所有env共享一个锁管理器
    lock: ObjectMapPathLock,

    // root级别的cache
    cache: ObjectMapRootCacheRef,

    // 所有托管的env
    all_envs: ObjectMapOpEnvContainer,
}

impl ObjectMapRootManager {
    pub fn new(
        owner: Option<ObjectId>,
        dec_id: Option<ObjectId>,
        noc: ObjectMapNOCCacheRef,
        root: ObjectMapRootHolder,
    ) -> Self {
        let lock = ObjectMapPathLock::new();
        let cache = ObjectMapRootMemoryCache::new_ref(noc, 60 * 5, 1024);
        Self {
            owner,
            dec_id,
            next_sid: AtomicU64::new(1),
            root,
            lock,
            cache,
            all_envs: ObjectMapOpEnvContainer::new(),
        }
    }

    fn next_sid(&self, op_env_type: ObjectMapOpEnvType) -> u64 {
        let sid = self
            .next_sid
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // 设置类型
        OpEnvSessionIDHelper::set_type(sid, op_env_type)
    }

    pub fn get_current_root(&self) -> ObjectId {
        self.root.get_current_root()
    }

    pub fn root_holder(&self) -> &ObjectMapRootHolder {
        &self.root
    }

    pub fn root_cache(&self) -> &ObjectMapRootCacheRef {
        &self.cache
    }

    pub fn managed_envs(&self) -> &ObjectMapOpEnvContainer {
        &self.all_envs
    }

    pub async fn create_op_env(&self) -> BuckyResult<ObjectMapPathOpEnvRef> {
        let sid = self.next_sid(ObjectMapOpEnvType::Path);
        let env = ObjectMapPathOpEnv::new(sid, &self.root, &self.lock, &self.cache).await;
        let env = ObjectMapPathOpEnvRef::new(env);

        Ok(env)
    }

    pub async fn create_managed_op_env(&self) -> BuckyResult<ObjectMapPathOpEnvRef> {
        let env = self.create_op_env().await?;

        self.all_envs.add_env(ObjectMapOpEnv::Path(env.clone()));

        Ok(env)
    }

    pub fn create_single_op_env(&self) -> BuckyResult<ObjectMapSingleOpEnvRef> {
        let sid = self.next_sid(ObjectMapOpEnvType::Single);
        let env = ObjectMapSingleOpEnv::new(
            sid,
            &self.root,
            &self.cache,
            self.owner.clone(),
            self.dec_id.clone(),
        );
        let env = ObjectMapSingleOpEnvRef::new(env);

        Ok(env)
    }

    pub fn create_managed_single_op_env(&self) -> BuckyResult<ObjectMapSingleOpEnvRef> {
        let env = self.create_single_op_env()?;
        self.all_envs.add_env(ObjectMapOpEnv::Single(env.clone()));

        Ok(env)
    }
}

pub type ObjectMapRootManagerRef = Arc<ObjectMapRootManager>;

mod test_root {
    use crate::*;
    use std::future::Future;

    async fn update_root<F, Fut>(update_root_fn: F) -> BuckyResult<()>
    where
        F: FnOnce(i32, i32) -> Fut,
        Fut: Future<Output = BuckyResult<i32>>,
    {
        info!("begin exec update fn...");
        let result = update_root_fn(1, 2).await?;
        info!("end exec update fn: {}", result);

        assert_eq!(result, 3);
        Ok(())
    }

    #[test]
    fn test_fn() {
        crate::init_simple_log("test-root-fn", Some("debug"));

        let update = |first: i32, second: i32| async move {
            info!("will exec add: {} + {}", first, second);
            Ok(first + second)
        };

        async_std::task::block_on(async move {
            update_root(update).await.unwrap();
        });
    }
}
