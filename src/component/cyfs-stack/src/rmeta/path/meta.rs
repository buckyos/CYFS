use super::access::*;
use super::config::*;
use super::link::*;
use cyfs_base::*;
use cyfs_lib::*;

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
pub struct GlobalStatePathMetaSyncCollection(Arc<NOCCollectionRWSync<GlobalStatePathMeta>>);

impl GlobalStatePathMetaSyncCollection {
    pub fn new(data: NOCCollectionRWSync<GlobalStatePathMeta>) -> Self {
        Self(Arc::new(data))
    }

    pub async fn add_access(&mut self, item: GlobalStatePathAccessItem) -> BuckyResult<bool> {
        {
            let mut meta = self.0.coll().write().unwrap();
            let ret = meta.access.add(item);
            if !ret {
                return Ok(false);
            }
        }

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(true)
    }

    pub async fn remove_access(
        &mut self,
        item: GlobalStatePathAccessItem,
    ) -> BuckyResult<Option<GlobalStatePathAccessItem>> {
        let ret = {
            let mut meta = self.0.coll().write().unwrap();
            let ret = meta.access.remove(item);
            if !ret.is_none() {
                return Ok(None);
            }

            ret
        };

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(ret)
    }

    pub async fn clear_access(
        &mut self,
    ) -> BuckyResult<bool> {
        {
            let mut meta = self.0.coll().write().unwrap();
            let ret = meta.access.clear();
            if !ret {
                return Ok(ret);
            }
        }

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(true)
    }

    pub fn check_access(&self) -> BuckyResult<()> {
        let meta = self.0.coll().read().unwrap();
        meta.access.check(req)
    }

    pub async fn add_link(
        &mut self,
        source: impl Into<String> + AsRef<str>,
        target: impl Into<String> + AsRef<str>,
    ) -> BuckyResult<bool> {
        {
            let mut meta = self.0.coll().write().unwrap();
            let ret = meta.link.add(source, target)?;
            if !ret {
                return Ok(false);
            }
        }

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(true)
    }

    pub async fn remove_link(&mut self, source: &str) -> BuckyResult<()> {
        {
            let mut meta = self.0.coll().write().unwrap();
            meta.link.remove(source)?;
        }

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(())
    }

    pub async fn clear_link(&mut self) -> BuckyResult<bool> {
        {
            let mut meta = self.0.coll().write().unwrap();
            let ret = meta.link.clear();
            if !ret {
                return Ok(ret);
            }
        }

        self.0.set_dirty(true);
        self.0.save().await?;

        Ok(true)
    }

    pub fn resolve_link(&self, source: &str) -> BuckyResult<Option<String>> {
        let meta = self.0.coll().read().unwrap();
        meta.link.resolve(source)
    }
}