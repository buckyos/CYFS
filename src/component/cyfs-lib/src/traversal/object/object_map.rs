use super::def::*;
use cyfs_base::*;

pub struct ObjectMapTraverser {
    cache: ObjectMapOpEnvCacheRef,
    current: NormalObject,
    cb: ObjectTraverserCallBackRef,
}

impl ObjectMapTraverser {
    pub fn new(
        cache: ObjectMapOpEnvCacheRef,
        current: NormalObject,
        cb: ObjectTraverserCallBackRef,
    ) -> Self {
        Self { cache, current, cb }
    }

    pub async fn tranverse(self) -> BuckyResult<()> {
        let target = self.current.object.object_id.clone();
        let mut visitor = ObjectMapFullVisitor::new(Box::new(self));
        visitor.visit(&target).await
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapTraverser {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        trace!("visit hub item: {}", item);

        let obj = SubObject {
            object_id: item.to_owned(),
        };
        let item = TraverseObjectItem::Sub(obj);
        self.cb.on_object(item).await
    }

    async fn visit_map_item(&mut self, key: &str, item: &ObjectId) -> BuckyResult<()> {
        trace!(
            "visit map item: {}={}, {:?}",
            key,
            item,
            item.obj_type_code()
        );

        let obj = self
            .current
            .derive_normal(item.to_owned(), Some(key), false);
        let item = TraverseObjectItem::Normal(obj);
        self.cb.on_object(item).await
    }

    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        trace!("visit set item: {}, {:?}", item, item.obj_type_code());

        let obj = self.current.derive_normal(item.to_owned(), None, false);
        let item = TraverseObjectItem::Normal(obj);
        self.cb.on_object(item).await
    }

    async fn visit_diff_map_item(
        &mut self,
        key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        trace!("visit diff map item: {}={}", key, item);

        // FIXME prev = altered - diff, should keep in touch here?;

        if let Some(id) = &item.diff {
            let obj = self.current.derive_normal(id.to_owned(), Some(key), false);
            let item = TraverseObjectItem::Normal(obj);
            self.cb.on_object(item).await?;
        }

        if let Some(altered) = &item.altered {
            let obj = self
                .current
                .derive_normal(altered.to_owned(), Some(key), false);
            let item = TraverseObjectItem::Normal(obj);
            self.cb.on_object(item).await?;
        }

        Ok(())
    }

    async fn visit_diff_set_item(&mut self, item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        trace!("visit diff set item: {}", item);

        if let Some(altered) = &item.altered {
            let obj = self.current.derive_normal(altered.to_owned(), None, false);
            let item = TraverseObjectItem::Normal(obj);
            self.cb.on_object(item).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitLoader for ObjectMapTraverser {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        self.cache.get_object_map(id).await
    }
}

impl ObjectMapVisitorProvider for ObjectMapTraverser {}
