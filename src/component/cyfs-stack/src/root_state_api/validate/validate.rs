use super::super::core::GlobalStateManager;
use super::cache::*;
use cyfs_base::*;

use std::sync::Arc;

#[derive(Debug)]
pub enum GlobalStateValidateRoot {
    GlobalRoot(ObjectId),
    DecRoot(ObjectId),
    None,
}

pub struct GlobalStateValidateRequest {
    pub dec_id: ObjectId,
    pub root: GlobalStateValidateRoot,
    pub inner_path: String,
    pub object_id: Option<ObjectId>,
}

impl std::fmt::Display for GlobalStateValidateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "root={:?}, dec={}, path={}, object={:?}",
            self.root, self.dec_id, self.inner_path, self.object_id
        )
    }
}

pub struct GlobalStateValidateResponse {
    pub dec_root_id: Option<ObjectId>,
    pub root_id: Option<ObjectId>,
    pub object_id: ObjectId,
}

enum CheckRoot {
    GlobalRoot,
    DecRoot(ObjectId),
    None,
}

struct GlobalStateValidator {
    root_state: Arc<GlobalStateManager>,
    op_env_cache: ObjectMapOpEnvCacheRef,

    cache: GlobalStatePathCache,
}

impl GlobalStateValidator {
    pub fn new(root_state: Arc<GlobalStateManager>) -> Self {
        let op_env_cache = ObjectMapOpEnvMemoryCache::new_ref(root_state.root_cache().clone());
        Self {
            root_state,
            op_env_cache,
            cache: GlobalStatePathCache::new(),
        }
    }

    async fn validate(
        &self,
        req: &GlobalStateValidateRequest,
    ) -> BuckyResult<GlobalStateValidateResponse> {
        let dec_root_id;
        let root_id;
        let check_root;

        // First try find the target from cache
        let cache_key = match &req.root {
            GlobalStateValidateRoot::GlobalRoot(global_root) => {
                dec_root_id = None;
                root_id = Some(global_root.to_owned());

                check_root = CheckRoot::GlobalRoot;

                let inner_path = format!("{}/{}", req.dec_id, req.inner_path);
                GlobalStatePathCacheKey {
                    root: global_root.clone(),
                    inner_path,
                }
            }
            GlobalStateValidateRoot::DecRoot(dec_root) => {
                dec_root_id = Some(dec_root.to_owned());
                root_id = None;

                check_root = CheckRoot::DecRoot(req.dec_id.clone());

                GlobalStatePathCacheKey {
                    root: dec_root.clone(),
                    inner_path: req.inner_path.clone(),
                }
            }
            GlobalStateValidateRoot::None => {
                let ret = self.root_state.get_dec_root(&req.dec_id).await?;
                if ret.is_none() {
                    let msg = format!("current dec root was not found! {}", req.dec_id);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                let info = ret.unwrap();

                dec_root_id = Some(info.2.clone());
                root_id = Some(info.0);

                check_root = CheckRoot::None;

                GlobalStatePathCacheKey {
                    root: info.2,
                    inner_path: req.inner_path.clone(),
                }
            }
        };

        let ret = self.cache.get(&cache_key)?;
        let target = if ret.is_none() {
            let ret = self.load_target(&cache_key, check_root).await?;
            if ret.is_none() {
                let msg = format!("the object referenced by path was not found! req={}", req);
                warn!("{}", msg);
                let err = BuckyError::new(BuckyErrorCode::NotFound, msg);
                self.cache.on_failed(cache_key, err.clone());

                return Err(err);
            }

            let target = ret.unwrap();
            self.cache.on_success(cache_key, target.clone());
            target
        } else {
            ret.unwrap()
        };

        // Check if target is matched
        match &req.object_id {
            Some(id) => {
                if target != *id {
                    let msg = format!(
                        "global state path validate unmatch! req={}, expect={}, got={}",
                        req, id, target
                    );
                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
                }

                info!("global state path validate success! req={}", req);
            }
            None => {
                info!(
                    "get object by global state path success! req={}, target={}",
                    req, target
                );
            }
        }

        let resp = GlobalStateValidateResponse {
            root_id,
            dec_root_id,
            object_id: target,
        };

        Ok(resp)
    }

    async fn check_global_root(&self, key: &GlobalStatePathCacheKey) -> BuckyResult<()> {
        let ret = self
            .root_state
            .root_cache()
            .get_object_map(&key.root)
            .await?;
        if ret.is_none() {
            let msg = format!("the specified global root was not found! {}", key.root);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let root = ret.unwrap();

        let obj = root.lock().await;
        if obj.class() != ObjectMapClass::GlobalRoot {
            let msg = format!(
                "the specified global root was not valid root objectmap! {}, class={:?}",
                key.root,
                obj.class()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        if obj.desc().dec_id().is_some() {
            let msg = format!(
                "the specified global root object's dec is not empty! {}, current_dec={:?}",
                key.root,
                obj.desc().dec_id()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(())
    }

    async fn check_dec_root(
        &self,
        key: &GlobalStatePathCacheKey,
        dec_id: &ObjectId,
    ) -> BuckyResult<()> {
        let ret = self
            .root_state
            .root_cache()
            .get_object_map(&key.root)
            .await?;
        if ret.is_none() {
            let msg = format!(
                "the specified dec_root was not found! {}, dec={}",
                key.root, dec_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let root = ret.unwrap();

        let obj = root.lock().await;
        if obj.class() != ObjectMapClass::DecRoot {
            let msg = format!(
                "the specified dec root was not valid dec root objectmap! {}, dec={}, class={:?}",
                key.root,
                dec_id,
                obj.class()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        if obj.desc().dec_id().as_ref() != Some(dec_id) {
            let msg = format!("the specified dec root object's dec not match the target dec! {}, current_dec={:?}, target_dec={}", 
            key.root, obj.desc().dec_id(), dec_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(())
    }

    async fn load_target(
        &self,
        key: &GlobalStatePathCacheKey,
        check_root: CheckRoot,
    ) -> BuckyResult<Option<ObjectId>> {
        // TODO add to failed cache for invalid root
        match check_root {
            CheckRoot::GlobalRoot => {
                self.check_global_root(key).await?;
            }
            CheckRoot::DecRoot(dec_id) => {
                self.check_dec_root(key, &dec_id).await?;
            }
            CheckRoot::None => {}
        }

        let path = ObjectMapPath::new(key.root.clone(), self.op_env_cache.clone());
        path.get_by_path(&key.inner_path).await
    }
}
