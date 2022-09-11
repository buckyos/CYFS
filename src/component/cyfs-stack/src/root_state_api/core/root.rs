use super::root_index::*;
use super::revision::*;
use cyfs_base::*;
use cyfs_lib::*;
use crate::config::StackGlobalConfig;

use std::sync::Arc;


// dec_root和关联的global_root信息
#[derive(Debug, Clone)]
pub struct DecRootInfo {
    pub dec_root: ObjectId,
    pub root: ObjectId,
}


// root objectmap
struct GlobalRootUpdateNotify {
    root_index: GlobalRootIndexRef,
}

#[async_trait::async_trait]
impl ObjectMapRootEvent for GlobalRootUpdateNotify {
    async fn root_updated(
        &self,
        dec_id: &Option<ObjectId>,
        new_root_id: ObjectId,
        prev_id: ObjectId,
    ) -> BuckyResult<()> {
        assert!(dec_id.is_none());
        self.root_index
            .update_root_state(new_root_id, Some(prev_id))
            .await
    }
}

// 聚合了所有dec state的objectmap作为全局状态
pub(crate) struct GlobalStateRoot {
    category: GlobalStateCategory,

    owner: Option<ObjectId>,

    root_index: GlobalRootIndexRef,

    // 这里递归的使用rootmanager来管理全局状态
    root: ObjectMapRootManager,

    noc_cache: ObjectMapNOCCacheRef,

    // 动态的revision映射管理器
    revision: RevisionList,

    // 访问模式
    config: StackGlobalConfig,
}

impl GlobalStateRoot {
    pub async fn load(
        category: GlobalStateCategory,
        device_id: &DeviceId,
        owner: Option<ObjectId>,
        noc: NamedObjectCacheRef,
        noc_cache: ObjectMapNOCCacheRef,
        config: StackGlobalConfig,
    ) -> BuckyResult<Self> {
        let revision = RevisionList::new();

        // 首先从noc加载global root的id
        let root_index = GlobalRootIndex::new(category.clone(), device_id, noc, revision.clone());
        let root_index = Arc::new(root_index);
        root_index.load().await?;

        // 如果第一次使用，需要初始化global root为空objectmap
        let mut root = root_index.get_root_state().root_state;
        if root.is_none() {
            

            // 初始化全局root对象
            let object_map = ObjectMap::new(ObjectMapSimpleContentType::Map, owner.clone(), None)
                .no_create_time().class(ObjectMapClass::GlobalRoot).build();

            let root_id = object_map.flush_id();

            info!("first init global state! category={}, owner={:?}, root={}", category, owner, root_id);

            // 需要立刻保存到noc
            noc_cache.put_object_map(root_id, object_map).await?;

            root_index.update_root_state(root_id, None).await?;
            root = Some(root_id);
        } else {
            info!("load global state success! category={}, root={}", category, root.as_ref().unwrap());

            /*
            // Compatibility code, remove later
            match Self::try_update_root_class(&noc_cache, root.as_ref().unwrap(), ObjectMapClass::GlobalRoot).await {
                Some(id) => root = Some(id),
                None => {},
            }
            */
        }

        // 创建基于global root的管理器，用以操作所有dec root状态的改变
        let root = root.unwrap();
        let notify = GlobalRootUpdateNotify {
            root_index: root_index.clone(),
        };
        let event = Arc::new(Box::new(notify) as Box<dyn ObjectMapRootEvent>);
        let root_holder = ObjectMapRootHolder::new(None, root, event);
        let root = ObjectMapRootManager::new(owner, None, noc_cache.clone(), root_holder);

        revision.update_dec_relation(&root).await?;
        
        let ret = Self {
            category,
            owner,
            root_index,
            root,
            noc_cache,
            revision,
            config,
        };

        Ok(ret)
    }

    pub fn category(&self) -> &GlobalStateCategory {
        &self.category
    }

    pub fn access_mode(&self) -> GlobalStateAccessMode {
        self.config.get_access_mode(self.category)
    }

    // direct changed the state, ignore access_mode
    pub(crate) async fn direct_set_root_state(&self, new_root_info: RootInfo, prev_root_id: Option<ObjectId>) -> BuckyResult<()> {
        assert!(new_root_info.root_state.is_some());

        self.root_index.direct_set_root_state(new_root_info.clone(), prev_root_id).await?;

        self.root.root_holder().direct_reload_root(new_root_info.root_state.unwrap()).await;

        self.revision.update_dec_relation(&self.root).await?;

        Ok(())
    }

    // 获取当前的全局根状态
    pub fn get_current_root(&self) -> (ObjectId, u64) {
        let root = self.root_index.get_root_state();
        (root.root_state.unwrap(), root.revision)
    }

    pub fn revision(&self) -> &RevisionList {
        &self.revision
    }

    pub fn root_cache(&self) -> &ObjectMapRootCacheRef {
        self.root.root_cache()
    }

    pub(super) async fn get_dec_root(
        &self,
        dec_id: &ObjectId,
        auto_create: bool,
    ) -> BuckyResult<Option<DecRootInfo>> {
        let key = dec_id.to_string();
        let env = self.root.create_op_env().await?;
        let root_id = env.get_by_key("/", &key).await.map_err(|e| {
            error!(
                "get dec root from global state error! category={}, dec={}, {}",
                self.category, dec_id, e
            );
            e
        })?;

        match root_id {
            Some(dec_root) => {
                info!(
                    "get dec root from global state! category={}, dec={}, dec_root={}",
                    self.category, dec_id, dec_root
                );

                /*
                // FIXME Compatibility code, remove later
                let info = match self.try_update_dec_root_class(dec_id, &dec_root).await {
                    Some(info) => info,
                    None => {
                        DecRootInfo {
                            dec_root,
                            root: env.root().to_owned(),
                        }
                    },
                };
                */

                let info = DecRootInfo {
                    dec_root,
                    root: env.root().to_owned(),
                };

                Ok(Some(info))
            }

            None => {
                if !auto_create {
                    return Ok(None);
                }

                if !self.access_mode().is_writable() {
                    let msg = format!("global state is in read mode: category={}", self.category);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                }
    
                // 创建一个空的objectmap
                let object_map = ObjectMap::new(
                    ObjectMapSimpleContentType::Map,
                    self.owner.clone(),
                    Some(dec_id.to_owned()),
                )
                .class(ObjectMapClass::DecRoot)
                .no_create_time()
                .build();
                let root_id = object_map.flush_id();

                info!("first create dec root! category={}, dec={}, root={}", self.category, dec_id, root_id);

                 // 需要立刻保存到noc
                self.noc_cache.put_object_map(root_id, object_map).await?;

                // 更新root状态并保存
                env.insert_with_key("/", &key, &root_id).await?;
                let global_root_id = env.commit().await.map_err(|e| {
                    error!(
                        "first create dec root but commit to global state root error! category={}, dec={}, dec_root={}",
                        self.category, dec_id, root_id
                    );
                    e
                })?;

                info!("first create dec root and commit to global state root success! category={}, dec={}, dec_new_root={}, global_root={}",
                self.category,
                dec_id, 
                root_id, 
                global_root_id);

                // 保存dec_root->global_root的映射
                self.revision.insert_dec_root(&dec_id, root_id.clone(), global_root_id.clone());

                let info = DecRootInfo {
                    dec_root: root_id,
                    root: global_root_id,
                };

                Ok(Some(info))
            }
        }
    }

    pub async fn update_dec_root(
        &self,
        dec_id: &ObjectId,
        new_root_id: ObjectId,
        prev_id: ObjectId,
    ) -> BuckyResult<ObjectId> {

        // first check access mode
        if !self.access_mode().is_writable() {
            let msg = format!("global state is in read mode: category={}", self.category);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        let key = dec_id.to_string();
        let env = self.root.create_op_env().await?;
        
        env.set_with_key("/", &key, &new_root_id, &Some(prev_id), false)
            .await
            .map_err(|e| {
                error!(
                    "update dec root to global root state error! category={}, dec={}, {}",
                    self.category, dec_id, e
                );
                e
            })?;
        
        let global_root_id = env.commit().await.map_err(|e| {
            error!(
                "update dec root but commit to global root error! category={}, dec={}, new_root={}, prev={}",
                self.category, dec_id, new_root_id, prev_id
            );
            e
        })?;

        info!("update dec root and commit to global root success! category={}, dec={}, dec_new_root={}, dec_prev={}, global_root={}",
            self.category,
            dec_id, 
            new_root_id, 
            prev_id, 
            global_root_id);

        // 保存dec_root->global_root的映射
        self.revision.insert_dec_root(&dec_id, new_root_id, global_root_id.clone());

        Ok(global_root_id)
    }

    /*
    // FIXME for beta version compatible, remove the two function later
    async fn try_update_root_class(noc_cache: &ObjectMapNOCCacheRef, root: &ObjectId, class: ObjectMapClass) -> Option<ObjectId> {
        let ret = noc_cache.get_object_map(root).await.unwrap();
        if ret.is_none() {
            unreachable!("root object not found! {}", root);
        }

        let mut obj = ret.unwrap();
        if obj.class() == class {
            return None;
        }

        obj.desc_mut().content_mut().set_class(class);
        let new_root_id = obj.flush_id();

        info!("update root object's class! {} -> {}, {:?}", root, new_root_id, class);

        // 需要立刻保存到noc
        noc_cache.put_object_map(new_root_id, obj).await.unwrap();

        Some(new_root_id)
    }

    async fn try_update_dec_root_class(&self, dec_id: &ObjectId, root: &ObjectId) -> Option<DecRootInfo> {
        let ret = Self::try_update_root_class(&self.noc_cache, root, ObjectMapClass::DecRoot).await;
        if ret.is_none() {
            return None;
        }

        let new_root_id = ret.unwrap();
        let global_root_id = self.update_dec_root(dec_id, new_root_id.clone(), root.to_owned()).await.unwrap();

        Some(DecRootInfo {
            dec_root: new_root_id,
            root: global_root_id,
        })
    }

    */
}

pub(crate) type GlobalStateRootRef = Arc<GlobalStateRoot>;