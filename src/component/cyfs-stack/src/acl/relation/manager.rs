use super::super::manager::AclMatchInstanceRef;
use super::super::request::*;
use super::cache::*;
use super::desc::*;
use super::AclSpecifiedRelation;
use cyfs_base::*;
use cyfs_debug::Mutex;

use futures::future::{AbortHandle, Aborted};
use once_cell::sync::OnceCell;
use std::sync::Arc;

// 延迟加载的relation
pub(crate) struct AclDelayRelation {
    description: AclRelationDescription,

    container: AclRelationContainer,

    // 这里持有cache的索引，里面的relation过期后可能被清理，需要重新向container发起get操作
    cache: OnceCell<AclRelationCacheRef>,
}

impl AclDelayRelation {
    pub fn new(container: AclRelationContainer, description: AclRelationDescription) -> Self {
        Self {
            container,
            cache: once_cell::sync::OnceCell::new(),
            description,
        }
    }

    pub fn desc(&self) -> &AclRelationDescription {
        &self.description
    }
}

#[async_trait::async_trait]
impl AclSpecifiedRelation for AclDelayRelation {
    async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        let ret = match self.cache.get() {
            Some(r) => {
                let opt = r.relation.read().unwrap().clone();
                match opt {
                    Some(v) => v,
                    None => {
                        // 过期了，需要重新get来加载内容
                        let _ = self
                            .container
                            .get(&self.description, req, Some(r.to_owned()))
                            .await;
                        r.relation.read().unwrap().as_ref().unwrap().clone()
                    }
                }
            }
            None => {
                // 第一次调用，需要向container发起请求
                let ret = self.container.get(&self.description, req, None).await;
                let _r = self.cache.set(ret.clone());
                self.cache
                    .get()
                    .unwrap()
                    .relation
                    .read()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .clone()
            }
        };

        match ret {
            Ok(v) => v.is_match(req).await,
            Err(e) => Err(e),
        }
    }
}

type AclDelayRelationRef = Arc<AclDelayRelation>;

#[derive(Clone)]
pub(crate) struct AclRelationManager {
    // 缓存
    container: AclRelationContainer,

    monitor_canceler: Arc<Mutex<Option<AbortHandle>>>,
}

impl AclRelationManager {
    pub fn new(match_instance: AclMatchInstanceRef) -> Self {
        let container = AclRelationContainer::new(match_instance);

        Self {
            container,
            monitor_canceler: Arc::new(Mutex::new(None)),
        }
    }

    pub fn load(&self, s: &str) -> BuckyResult<AclDelayRelation> {
        let desc = AclRelationDescription::load(s)?;
        let delay_relation = AclDelayRelation::new(self.container.clone(), desc);
        // let ret = Arc::new(Box::new(delay_relation) as Box<dyn AclSpecifiedRelation>);

        Ok(delay_relation)
    }

    pub fn start_monitor(&self) {
        let container = self.container.clone();

        let (release_task, handle) = futures::future::abortable(async move {
            loop {
                async_std::task::sleep(std::time::Duration::from_secs(60)).await;

                container.gc();
            }
        });

        {
            let mut v = self.monitor_canceler.lock().unwrap();
            assert!(v.is_none());
            *v = Some(handle);
        }

        async_std::task::spawn(async move {
            match release_task.await {
                Ok(_) => {
                    info!("acl relation manager monitor complete",);
                }
                Err(Aborted) => {
                    info!("acl relation manager monitor cancelled",);
                }
            }
        });
    }

    pub fn stop_monitor(&self) {
        info!("will stop acl relation manager monitor");
        let handle = self.monitor_canceler.lock().unwrap().take();
        if let Some(handle) = handle {
            handle.abort();
        }
    }
}
