use super::data::ChunksCollector;
use cyfs_base::*;

use async_std::channel::{Receiver, Sender};
use std::collections::{HashSet, VecDeque};

#[derive(Debug)]
enum WalkMsg {
    Continue(ObjectId),
    Break(ObjectId),
    Wait,
}

#[derive(Clone)]
struct WalkPendingItem {
    item: ObjectId,
    retry_count: u8,
}

#[derive(Clone)]
pub(super) struct ObjectMapWalker {
    target: ObjectId,
    cache: ObjectMapOpEnvCacheRef,
    pending_items: VecDeque<WalkPendingItem>,

    chunks_collector: ChunksCollector,

    // 对于tx端，用以单次next里面去重
    result: Vec<ObjectId>,
    // 对于tx端，用以单次sync里面，判断哪些object同步成功但缺失
    missing_list: HashSet<ObjectId>,

    tx: Sender<WalkMsg>,
    rx: Receiver<WalkMsg>,
}

impl ObjectMapWalker {
    pub fn new(
        cache: ObjectMapOpEnvCacheRef,
        target: ObjectId,
        chunks_collector: ChunksCollector,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded::<WalkMsg>(1);

        Self {
            target,
            cache,
            chunks_collector,
            pending_items: VecDeque::new(),
            result: vec![],
            missing_list: HashSet::new(),
            tx,
            rx,
        }
    }

    fn first_pend_item(&mut self, item: ObjectId) {
        let v = WalkPendingItem {
            item,
            retry_count: 0,
        };
        self.pending_items.push_back(v);
    }

    async fn on_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        trace!("walk on item: {}", item);

        if item.obj_type_code() == ObjectTypeCode::Chunk {
            if let Err(e) = self.chunks_collector.append(item).await {
                error!("walk object's chunks error! {}, {}", item, e);

                // TODO 出错情况下，如何处理该对象？忽略还是需要返回错误？
            }

            return Ok(());
        }

        // 这里要做一次去重
        if self.result.iter().find(|&&v| v == *item).is_some() {
            return Ok(());
        }

        let ret = self.cache.exists(item).await;
        match ret {
            Ok(exists) => {
                if exists {
                    if let Err(e) = self.chunks_collector.append(item).await {
                        error!("walk object's chunks error! {}, {}", item, e);

                        // TODO 出错情况下，如何处理该对象？忽略还是需要返回错误？
                    }

                    return Ok(());
                }

                // 缓存结果，用以去重
                self.result.push(item.to_owned());
                if let Err(e) = self.tx.send(WalkMsg::Continue(item.to_owned())).await {
                    let msg = format!(
                        "objectmap walker send error! target={}, item={}, {}",
                        self.target, item, e
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
                }
            }
            Err(e) => {
                error!(
                    "exists objectmap' item error! target={}, id={}, {}",
                    self.target, item, e
                );
                // TODO 出错情况下，如何处理该对象？忽略还是放入同步列表？
            }
        }

        Ok(())
    }

    pub async fn next(&self, count: usize) -> Vec<ObjectId> {
        let mut list = vec![];
        // trace!("walk next, count={}", count);

        loop {
            match self.rx.recv().await {
                Ok(item) => {
                    match item {
                        WalkMsg::Continue(item) => {
                            trace!("walk recv continue: {}", item);

                            if list.iter().find(|&&v| v == item).is_none() {
                                list.push(item);
                            }

                            if list.len() >= count {
                                break;
                            }
                        }
                        WalkMsg::Break(item) => {
                            trace!("walk recv break: {}", item);

                            if list.iter().find(|&&v| v == item).is_none() {
                                list.push(item);
                            }

                            assert!(list.len() > 0);
                            // 需要同步对象后才可以继续
                            break;
                        }
                        WalkMsg::Wait => {
                            continue;
                        }
                    }
                }
                Err(e) => {
                    info!(
                        "objectmap walk recv error, now will break! target={}, {}",
                        self.target, e
                    );
                    break;
                }
            }
        }

        debug!("walk next will return: {:?}", list);

        list
    }

    pub fn start(mut self) {
        async_std::task::spawn(async move {
            let _ = self.visit(self.target.clone()).await;
        });
    }

    async fn visit(&mut self, target: ObjectId) -> BuckyResult<()> {
        self.first_pend_item(target.clone());

        loop {
            let cur = self.pending_items.pop_front();
            if cur.is_none() {
                break;
            }

            let WalkPendingItem { item, retry_count } = cur.unwrap();

            if self.missing_list.contains(&item) {
                continue;
            }

            let ret = self.get_object_map(&item).await?;
            match ret {
                Some(obj) => {
                    let obj_item = obj.lock().await;
                    debug!("will visit objectmap item: {}", item);
                    obj_item.visit(self).await?;
                }
                None => {
                    // 如果需要遍历的对象本地不存在的话，那么需要尝试同步一次，暂停当前的遍历
                    // 如果经过一次重试也不存在，那么忽略继续
                    if retry_count == 0 {
                        debug!("visit item but not found! now will sync item: {}", item);

                        let v = WalkPendingItem {
                            item: item.clone(),
                            retry_count: retry_count + 1,
                        };
                        self.pending_items.push_front(v);

                        if let Err(e) = self.tx.send(WalkMsg::Break(item)).await {
                            error!("objectmap visit send break error! target={}, {}", target, e);
                            break;
                        }
                        self.result.clear();

                        // send two wait msg so we can wait the result
                        if let Err(e) = self.tx.send(WalkMsg::Wait).await {
                            error!("objectmap visit send wait error! target={}, {}", target, e);
                            break;
                        }
                        if let Err(e) = self.tx.send(WalkMsg::Wait).await {
                            error!("objectmap visit send wait error! target={}, {}", target, e);
                            break;
                        }
                    } else {
                        warn!(
                            "visit item retry but still not found! now will ignore item: {}",
                            item
                        );
                        self.missing_list.insert(item);
                    }
                }
            }
        }

        info!("objectmap walker complete, target={}", target);
        self.tx.close();

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitor for ObjectMapWalker {
    async fn visit_hub_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        trace!("visit hub item: {}", item);
        self.first_pend_item(item.to_owned());

        self.on_item(item).await?;

        Ok(())
    }

    async fn visit_map_item(&mut self, key: &str, item: &ObjectId) -> BuckyResult<()> {
        trace!(
            "visit map item: {}={}, {:?}",
            key,
            item,
            item.obj_type_code()
        );

        if item.obj_type_code() == ObjectTypeCode::ObjectMap {
            self.first_pend_item(item.to_owned());
        }

        self.on_item(item).await?;

        Ok(())
    }

    async fn visit_set_item(&mut self, item: &ObjectId) -> BuckyResult<()> {
        trace!("visit set item: {}, {:?}", item, item.obj_type_code());

        if item.obj_type_code() == ObjectTypeCode::ObjectMap {
            self.first_pend_item(item.to_owned());
        }

        self.on_item(item).await?;

        Ok(())
    }

    async fn visit_diff_map_item(
        &mut self,
        key: &str,
        item: &ObjectMapDiffMapItem,
    ) -> BuckyResult<()> {
        trace!("visit diff map item: {}={}", key, item);

        if let Some(id) = &item.diff {
            self.first_pend_item(id.to_owned());
            self.on_item(id).await?;
        }

        if let Some(altered) = &item.altered {
            self.on_item(altered).await?;
        }

        Ok(())
    }

    async fn visit_diff_set_item(&mut self, item: &ObjectMapDiffSetItem) -> BuckyResult<()> {
        trace!("visit diff set item: {}", item);

        if let Some(altered) = &item.altered {
            self.on_item(altered).await?;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectMapVisitLoader for ObjectMapWalker {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }

    async fn get_object_map(&mut self, id: &ObjectId) -> BuckyResult<Option<ObjectMapRef>> {
        self.cache.get_object_map(id).await
    }
}
