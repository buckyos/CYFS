use crate::named_object_storage::*;
use cyfs_base::{bucky_time_now, BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_lib::*;

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub(crate) trait NOCUpdaterProvider: Sync + Send + 'static {
    async fn try_get(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectCacheData>>;
    async fn update_signs(&self, req: &ObjectCacheData, insert_time: &u64) -> BuckyResult<usize>;
    async fn replace_old(&self, req: &ObjectCacheData, old: &ObjectCacheData)
        -> BuckyResult<usize>;
    async fn insert_new(&self, req: &ObjectCacheData) -> BuckyResult<usize>;
}

pub(crate) struct NOCUpdater {
    insert_object_event: InsertObjectEventManager,
}

impl NOCUpdater {
    pub fn new(insert_object_event: InsertObjectEventManager) -> Self {
        Self {
            insert_object_event,
        }
    }

    // 更新签名
    async fn merge_signs(
        &self,
        updater: &impl NOCUpdaterProvider,
        mut current: ObjectCacheData,
        req: &ObjectCacheData,
        event: &Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<Option<usize>> {
        use cyfs_base::RawConvertTo;

        // 必须是update_time一致情况下，才会尝试合并签名
        assert_eq!(current.update_time, req.update_time);

        assert!(current.object.is_some());
        assert!(req.object.is_some());

        // 确保只有当前一个引用
        let mut current_obj = current.object.take().unwrap();
        assert!(Arc::strong_count(&current_obj) == 1);

        let current_obj_mut = Arc::get_mut(&mut current_obj).unwrap();
        let current_signs = current_obj_mut.signs_mut().unwrap();
        let req_signs = req.object.as_ref().unwrap().signs().unwrap();

        let ret = current_signs.merge(req_signs);
        if ret == 0 {
            // 不需要更新签名，那么直接认为成功
            return Ok(None);
        }

        current.object = Some(current_obj);

        info!(
            "object signs updated! id={}, count={}",
            current.object_id, ret
        );
        let object_raw = current.object.as_ref().unwrap().to_vec().map_err(|e| {
            let msg = format!(
                "encode object with updated signs failed! obj={}, {}",
                current.object_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        current.object_raw = Some(object_raw);

        // 触发pre_put事件
        if let Some(event) = &event {
            if let Err(e) = event.pre_put(&current, false).await {
                warn!(
                    "update signs and put to noc cancelled by event: obj={}, {}",
                    req.object_id, e
                );
                return Err(e);
            }
        }

        let mut insert_time = bucky_time_now();

        // 本地时钟可能回滚，需要确保插入时间大于旧数据的插入时间
        if insert_time < current.insert_time {
            warn!(
                "time_now is smaller than current one! cur={}, old={}",
                insert_time, current.insert_time
            );
            insert_time = current.insert_time + 1;
        }

        let count = updater.update_signs(&current, &insert_time).await?;
        if count > 0 {
            // 更新成功后，触发相应的事件
            current.insert_time = insert_time;
            let _ = self.insert_object_event.emit(&current);

            // 触发post_put事件
            if let Some(event) = &event {
                if let Err(e) = event.post_put(&current, false).await {
                    warn!(
                        "update signs and post put to noc return error: obj={}, {}",
                        req.object_id, e
                    );
                    // FIXME post_put是否会根据结果返回对应的错误？
                    return Err(e);
                }
            }
        }

        Ok(Some(count))
    }

    pub async fn update(
        &self,
        updater: &impl NOCUpdaterProvider,
        req: &ObjectCacheData,
        event: Option<Box<dyn NamedObjectStorageEvent>>,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        assert!(req.object_raw.is_some());

        let object = req.object.as_ref().unwrap();
        let mut resp = NamedObjectCacheInsertResponse::new(NamedObjectCacheInsertResult::Accept);
        resp.set_times(object);

        let mut retry_count = 0;
        loop {
            // 为了避免一些极端情况陷入死循环
            retry_count += 1;
            if retry_count > 16 {
                let msg = format!(
                    "udpate object extend max retry count! obj={}",
                    req.object_id
                );
                error!("{}", msg);

                break Err(BuckyError::from(msg));
            }

            // 首先查找是否已经存在
            match updater.try_get(&req.object_id).await {
                Ok(Some(old)) => {
                    assert!(old.update_time > 0);
                    assert!(req.update_time > 0);

                    // update_time相同情况下，尝试合并签名
                    let cur_update_time = old.update_time;
                    let new_udpate_time = req.update_time;
                    if new_udpate_time == cur_update_time {
                        info!(
                            "insert obj but update_time is same, now will update signs: obj={}, cur={}",
                            req.object_id, cur_update_time,
                        );

                        let old_insert_time = old.insert_time;
                        match self.merge_signs(updater, old, req, &event).await {
                            Ok(merge_ret) => {
                                match merge_ret {
                                    Some(count) => {
                                        // 签名需要更新，但可能会发生竞争导致失败，需要判断count
                                        // count=0表示发生了竞争，需要重试
                                        if count == 0 {
                                            warn!(
                                                "update signs but not found, now will retry! obj={} cur_insert_time={}",
                                                req.object_id, old_insert_time
                                            );
                                            continue;
                                        }

                                        info!(
                                            "update signs success! obj={} old_insert_time={}",
                                            req.object_id, old_insert_time
                                        );

                                        resp.result = NamedObjectCacheInsertResult::Merged;
                                        break Ok(resp);
                                    }
                                    None => {
                                        debug!(
                                            "signs not changed! obj={} old_insert_time={}",
                                            req.object_id, old_insert_time
                                        );

                                        // 签名不需要更新
                                        resp.result = NamedObjectCacheInsertResult::AlreadyExists;
                                        break Ok(resp);
                                    }
                                }
                            }
                            Err(e) => {
                                // 更新签名失败，直接终止操作
                                break Err(e);
                            }
                        }
                    } else if new_udpate_time < cur_update_time {
                        info!(
                            "insert obj but update_time is same or older: obj={}, cur={}, new={}",
                            req.object_id, cur_update_time, new_udpate_time
                        );

                        // 如果对象已经存在，返回AlreadyExists和旧对象的时间
                        resp.result = NamedObjectCacheInsertResult::AlreadyExists;
                        resp.set_times(old.object.as_ref().unwrap());

                        break Ok(resp);
                    }

                    info!(
                        "will replace obj: obj={}, cur={}, new={}",
                        req.object_id, cur_update_time, new_udpate_time
                    );

                    // 触发pre_put事件
                    if let Some(event) = &event {
                        if let Err(e) = event.pre_put(&req, false).await {
                            warn!(
                                "update put to noc cancelled by event: obj={}, {}",
                                req.object_id, e
                            );
                            break Err(e);
                        }
                    }

                    match updater.replace_old(req, &old).await {
                        Ok(count) => {
                            // 如果查找不到，说明发生了竞争，需要重试
                            if count != 1 {
                                warn!(
                                    "replace but not found, now will retry! obj={} cur_update_time={}",
                                    req.object_id, cur_update_time
                                );
                                continue;
                            }
                            debug!(
                                "replace obj success! obj={} cur={} new={}",
                                req.object_id, cur_update_time, new_udpate_time
                            );

                            // 更新成功后，触发相应的事件
                            let _ = self.insert_object_event.emit(&req);

                            // 触发post_put事件
                            if let Some(event) = &event {
                                if let Err(e) = event.post_put(&req, false).await {
                                    warn!(
                                        "update post put to noc return error: obj={}, {}",
                                        req.object_id, e
                                    );
                                    // FIXME post_put是否会根据结果返回对应的错误？
                                    break Err(e);
                                }
                            }

                            resp.result = NamedObjectCacheInsertResult::Updated;
                            break Ok(resp);
                        }
                        Err(e) => {
                            // FIXME 出错后需要重试与否？
                            let msg =
                                format!("replace object failed! obj={}, err={}", req.object_id, e);
                            error!("{}", msg);
                            break Err(BuckyError::from(msg));
                        }
                    }
                }

                Ok(None) => {
                    // 不存在，尝试全新插入
                    info!("will insert new object: {}", req.object_id);

                    // 触发pre_put事件
                    if let Some(event) = &event {
                        if let Err(e) = event.pre_put(&req, true).await {
                            warn!(
                                "first put to noc cancelled by event: obj={}, {}",
                                req.object_id, e
                            );
                            break Err(e);
                        }
                    }

                    let ret = updater.insert_new(req).await;

                    match ret {
                        Ok(_) => {
                            // 插入成功后，触发相应的事件
                            let _ = self.insert_object_event.emit(&req);
                            // 触发post_put事件
                            if let Some(event) = &event {
                                if let Err(e) = event.post_put(&req, true).await {
                                    warn!(
                                        "first post put to noc return error: obj={}, {}",
                                        req.object_id, e
                                    );

                                    // FIXME post_put是否会根据结果返回对应的错误？
                                    break Err(e);
                                }
                            }

                            break Ok(resp);
                        }
                        Err(e) => {
                            if e.code() == BuckyErrorCode::AlreadyExists {
                                // 插入时候发生了竞争,需要再次尝试
                                warn!(
                                    "insert but already exists! will retry! obj={}",
                                    req.object_id
                                );
                                continue;
                            } else {
                                break Err(e);
                            }
                        }
                    }
                }

                Err(e) => {
                    // 查询出错，直接终止
                    error!("get obj failed! obj={}, err={}", req.object_id, e);
                    break Err(e);
                }
            }
        }
    }
}
