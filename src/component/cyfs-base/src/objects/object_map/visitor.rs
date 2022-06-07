use super::object_map::*;
use crate::*;

use std::any::Any;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ObjectMapVisitor: Send + Sync {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()>;
    async fn visit_map_item(&mut self, key: &str, item: &ObjectId) -> BuckyResult<()>;
    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()>;
    async fn visit_diff_map_item(
        &mut self,
        key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()>;
    async fn visit_diff_set_item(&mut self, item: &ObjectMapDiffSetItem) -> BuckyResult<()>;
}

// objectmap loader for visitor, change the behavior for default cache(memory and noc, etc.)
#[async_trait::async_trait]
pub trait ObjectMapVisitLoader: Send + Sync {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>>;
}

pub type ObjectMapVisitorRef = Arc<Box<dyn ObjectMapVisitor>>;

pub trait ObjectMapVisitorProvider: ObjectMapVisitor + ObjectMapVisitLoader {}

// visit objectmap's all leaf nodes and hub nodes
pub struct ObjectMapFullVisitor {
    provider: Box<dyn ObjectMapVisitorProvider>,
    pending_items: std::collections::VecDeque<ObjectId>,
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapFullVisitor {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        self.pending_items.push_back(item.to_owned());

        self.provider.visit_hub_item(item).await
    }

    async fn visit_map_item(&mut self, key: &str, item: &ObjectId) -> BuckyResult<()> {
        self.provider.visit_map_item(key, item).await
    }

    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        self.provider.visit_set_item(item).await
    }

    async fn visit_diff_map_item(
        &mut self,
        key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        self.provider.visit_diff_map_item(key, item).await
    }

    async fn visit_diff_set_item(&mut self, item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        self.provider.visit_diff_set_item(item).await
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitLoader for ObjectMapFullVisitor {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        self.provider.get_object_map(id).await
    }
}

impl ObjectMapVisitorProvider for ObjectMapFullVisitor {}

impl ObjectMapFullVisitor {
    pub fn new(provider: Box<dyn ObjectMapVisitorProvider>) -> Self {
        Self {
            provider,
            pending_items: std::collections::VecDeque::new(),
        }
    }

    pub fn into_provider(self) -> Box<dyn ObjectMapVisitorProvider> {
        self.provider
    }

    pub async fn visit(&mut self, target: &ObjectId) -> BuckyResult<()> {
        self.pending_items.push_back(target.to_owned());

        loop {
            let cur = self.pending_items.pop_front();
            if cur.is_none() {
                break;
            }

            let cur = cur.unwrap();
            let ret = self.provider.get_object_map(&cur).await?;
            match ret {
                Some(obj) => {
                    let obj_item = obj.lock().await;
                    debug!("will visit full item: {}", cur);
                    obj_item.visit(self).await?;
                }
                None => {
                    debug!("visit full item: but not found! {}", cur);
                    continue;
                }
            }
        }

        Ok(())
    }
}

pub struct ObjectMapDummyVisitor;

impl ObjectMapDummyVisitor {
    pub fn new() -> Self {
        ObjectMapDummyVisitor {}
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapDummyVisitor {
    async fn visit_hub_item(&mut self, _item: &ObjectId) -> BuckyResult<()> {
        Ok(())
    }

    async fn visit_map_item(&mut self, _key: &str, _item: &ObjectId) -> BuckyResult<()> {
        Ok(())
    }

    async fn visit_set_item(&mut self, _item: &ObjectId) -> BuckyResult<()> {
        Ok(())
    }

    async fn visit_diff_map_item(
        &mut self,
        _key: &str,
        _item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        Ok(())
    }

    async fn visit_diff_set_item(&mut self, _item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        Ok(())
    }
}

// for objectmap path env, visitor for objectmap tree
pub struct ObjectMapPathVisitor {
    provider: Box<dyn ObjectMapVisitorProvider>,
    pending_items: std::collections::VecDeque<ObjectMapRef>,
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapPathVisitor {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        self.pend_sub(item).await?;

        self.provider.visit_hub_item(item).await
    }

    async fn visit_map_item(&mut self, key: &str, item: &ObjectId) -> BuckyResult<()> {
        debug!(
            "visit map item: {}={}, type={:?}",
            key,
            item,
            item.obj_type_code()
        );

        if item.obj_type_code() == ObjectTypeCode::ObjectMap {
            self.pend_sub(item).await?;
        }

        self.provider.visit_map_item(key, item).await
    }

    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        debug!("visit set item: {}", item);

        if item.obj_type_code() == ObjectTypeCode::ObjectMap {
            self.pend_sub(item).await?;
        }

        self.provider.visit_set_item(item).await
    }

    async fn visit_diff_map_item(
        &mut self,
        key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        // expand sub diff item only
        if let Some(id) = &item.diff {
            self.pend_sub(id).await?;
        }

        self.provider.visit_diff_map_item(key, item).await
    }

    async fn visit_diff_set_item(&mut self, item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        self.provider.visit_diff_set_item(item).await
    }
}

impl ObjectMapPathVisitor {
    pub fn new(provider: Box<dyn ObjectMapVisitorProvider>) -> Self {
        Self {
            provider,
            pending_items: std::collections::VecDeque::new(),
        }
    }

    async fn pend_sub(&mut self, id: &ObjectId) -> BuckyResult<()> {
        assert_eq!(id.obj_type_code(), ObjectTypeCode::ObjectMap);

        if let Some(item) = self.provider.get_object_map(&id).await? {
            self.pending_items.push_back(item)
        }

        Ok(())
    }

    pub fn into_provider(self) -> Box<dyn ObjectMapVisitorProvider> {
        self.provider
    }

    pub async fn visit(&mut self, target: &ObjectId) -> BuckyResult<()> {
        self.pend_sub(target).await?;

        loop {
            let cur = self.pending_items.pop_front();
            if cur.is_none() {
                break;
            }

            let cur = cur.unwrap();

            let obj_item = cur.lock().await;
            debug!("will visit path item: {:?}", obj_item.cached_object_id());

            obj_item.visit(self).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitLoader for ObjectMapPathVisitor {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        self.provider.get_object_map(id).await
    }
}

impl ObjectMapVisitorProvider for ObjectMapPathVisitor {}