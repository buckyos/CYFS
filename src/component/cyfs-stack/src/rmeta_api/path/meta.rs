use super::access::*;
use super::config::*;
use super::link::*;
use cyfs_base::*;
use cyfs_lib::*;
use super::storage::*;

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStatePathMeta {
    access: GlobalStatePathAccessList,
    link: GlobalStatePathLinkList,
    config: GlobalStatePathConfigList,
}

impl Default for GlobalStatePathMeta {
    fn default() -> Self {
        Self {
            access: GlobalStatePathAccessList::default(),
            link: GlobalStatePathLinkList::default(),
            config: GlobalStatePathConfigList::default(),
        }
    }
}

declare_collection_codec_for_serde!(GlobalStatePathMeta);


#[derive(Clone)]
pub struct GlobalStatePathMetaSyncCollection{
    meta: Arc<NOCCollectionRWSync<GlobalStatePathMeta>>,
    
    // dump to local file for debug and review
    storage: Arc<GlobalStatePathMetaStorage>,
}

impl GlobalStatePathMetaSyncCollection {
    pub fn new(storage: Arc<GlobalStatePathMetaStorage>, meta: NOCCollectionRWSync<GlobalStatePathMeta>) -> Self {
        
        Self {
            meta: Arc::new(meta),
            storage,
        }
    }

    fn dump(&self) {
        let data = {
            let meta = self.meta.coll().read().unwrap();
            serde_json::to_string(&meta as &GlobalStatePathMeta).unwrap()
        };

        let storage = self.storage.clone();
        async_std::task::spawn(async move {
            storage.save(data).await
        });
    }

    pub async fn add_access(&self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        {
            let mut meta = self.meta.coll().write().unwrap();
            let ret = meta.access.add(item);
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;
        
        self.dump();

        Ok(true)
    }

    pub async fn remove_access(
        &self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>> {
        let ret = {
            let mut meta = self.meta.coll().write().unwrap();
            let ret = meta.access.remove(item);
            if !ret.is_none() {
                return Ok(None);
            }

            ret
        };

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump();

        Ok(ret)
    }

    pub async fn clear_access(
        &self,
    ) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().unwrap();
            meta.access.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump();

        Ok(ret)
    }

    pub fn check_access<'d, 'a, 'b>(&self, req: GlobalStateAccessRequest<'d, 'a, 'b>) -> BuckyResult<()> {
        let meta = self.meta.coll().read().unwrap();
        meta.access.check(req)
    }

    pub async fn add_link(
        &self,
        source: impl Into<String> + AsRef<str>,
        target: impl Into<String> + AsRef<str>,
    ) -> BuckyResult<bool> {
        {
            let mut meta = self.meta.coll().write().unwrap();
            let ret = meta.link.add(source, target)?;
            if !ret {
                return Ok(false);
            }
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump();

        Ok(true)
    }

    pub async fn remove_link(&self, source: &str) -> BuckyResult<Option<GlobalStatePathLinkItem>> {
        let ret = {
            let mut meta = self.meta.coll().write().unwrap();
            meta.link.remove(source)?
        };

        if ret.is_none() {
            return Ok(None);
        }
        
        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump();

        Ok(ret)
    }

    pub async fn clear_link(&self) -> BuckyResult<usize> {
        let ret = {
            let mut meta = self.meta.coll().write().unwrap();
            meta.link.clear()
        };

        if ret == 0 {
            return Ok(ret);
        }

        self.meta.set_dirty(true);
        self.meta.save().await?;

        self.dump();

        Ok(ret)
    }

    pub fn resolve_link(&self, source: &str) -> BuckyResult<Option<String>> {
        let meta = self.meta.coll().read().unwrap();
        meta.link.resolve(source)
    }
}