use super::obj_searcher::*;
use cyfs_base::*;

use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Clone)]
struct ObjectSearcherWrapper(Arc<dyn ObjectSearcher>);

#[derive(Clone)]
pub(crate) struct OodResolver {
    device_id: DeviceId,
    searcher: ObjectSearcherRef,
}

// 依赖下面几个核心要素
// Device如果没有owner，那么就是本身，否则需要查询owner
// SimpleGroup,UnioinAccount,Tx没有Owner
// 目前认为People也没有owner
// People,SimpleGroup 对象存在ood_list
impl OodResolver {
    pub(crate) fn new(local_device_id: DeviceId, searcher: ObjectSearcherRef) -> Self {
        Self {
            device_id: local_device_id,
            searcher,
        }
    }

    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    pub async fn resolve_ood(
        &self,
        object_id: &ObjectId,
        owner_id: Option<ObjectId>,
    ) -> BuckyResult<Vec<DeviceId>> {
        let mut object = None;

        // 首先本地查询此对象
        match self.searcher.search(owner_id.clone(), object_id).await {
            Ok(data) => {
                debug!("get object from local cache: {}", object_id);

                object = data.object;
            }
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    error!(
                        "get object from local cache and meta-chain but not found: {}",
                        object_id
                    );
                } else {
                    error!("get object from local cache error: {} {}", object_id, e);
                }
            }
        };

        if object.is_some() {
            self.get_ood_by_object(object_id.clone(), owner_id, object.unwrap())
                .await
        } else {
            self.get_ood_by_object_with_owner(object_id.clone(), owner_id)
                .await
        }
    }

    pub async fn get_ood_by_object(
        &self,
        object_id: ObjectId,
        owner_id: Option<ObjectId>,
        object: Arc<AnyNamedObject>,
    ) -> BuckyResult<Vec<DeviceId>> {
        Self::get_ood_by_object_impl(self.searcher.clone(), object_id, owner_id, object).await
    }

    async fn get_ood_by_object_with_owner(
        &self,
        object_id: ObjectId,
        owner_id: Option<ObjectId>,
    ) -> BuckyResult<Vec<DeviceId>> {
        Self::get_ood_by_object_with_owner_impl(self.searcher.clone(), object_id, owner_id).await
    }

    fn append_device_id(device_list: &mut Vec<DeviceId>, device_id: DeviceId) {
        if !device_list.iter().any(|id| device_id == *id) {
            device_list.push(device_id);
        }
    }

    fn append_device_list(device_list: &mut Vec<DeviceId>, list: &Vec<DeviceId>) {
        list.iter()
            .for_each(|device_id| Self::append_device_id(device_list, device_id.clone()));
    }

    async fn get_ood_by_object_impl(
        searcher: ObjectSearcherRef,
        mut object_id: ObjectId,
        mut owner_id: Option<ObjectId>,
        mut object: Arc<AnyNamedObject>,
    ) -> BuckyResult<Vec<DeviceId>> {
        let mut device_list: Vec<DeviceId> = Vec::new();
        let ret = loop {
            let obj_type = object_id.obj_type_code();

            // People,SimpleGroup 对象存在ood_list
            if obj_type == ObjectTypeCode::People || obj_type == ObjectTypeCode::SimpleGroup {
                match object.ood_list() {
                    Ok(list) => {
                        if list.len() > 0 {
                            debug!(
                                "get ood list from object ood_list: {} {:?}",
                                object_id, list
                            );

                            Self::append_device_list(&mut device_list, &list);

                            break Ok(());
                        } else {
                            let msg =
                                format!("get ood list from object ood_list empty: {}", object_id);
                            warn!("{}", msg);

                            break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                        }
                    }
                    Err(e) => {
                        // 如果没指定ood_list，由于又不存在owner，终止查找
                        warn!("get ood_list from object failed! {} {}", object_id, e);
                        break Err(e);
                    }
                }
            }

            // 如果没有强制指定owner，那么尝试从object获取owner
            if owner_id.is_none() {
                owner_id = object.owner().clone();
            }

            // 其余类型，尝试获取其owner
            if let Some(cur_owner_id) = owner_id {
                // 如果owner就是device，那么返回这个device，不需要再继续查找
                if cur_owner_id.obj_type_code() == ObjectTypeCode::Device {
                    info!("owner is device: obj={}, owner={}", object_id, cur_owner_id);

                    let device_id = DeviceId::try_from(cur_owner_id).unwrap();
                    Self::append_device_id(&mut device_list, device_id);

                    break Ok(());
                }

                // owner不能为自己
                if object_id == cur_owner_id {
                    let msg = format!(
                        "object invalid owner: obj={}, owner={}",
                        object_id, cur_owner_id
                    );
                    warn!("{}", msg);

                    break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }

                // 从owner出发，再次查找对象
                debug!(
                    "will search for owner: obj={}, owner={}",
                    object_id, cur_owner_id
                );

                match searcher.search(None, &cur_owner_id).await {
                    Ok(ret) => {
                        // 递归调用会出很多问题，这里改用循环替代
                        // Self::get_ood_by_object_impl(searcher, object_id, object).await

                        object_id = cur_owner_id.clone();
                        owner_id = None;
                        object = ret.object.unwrap();

                        continue;
                    }
                    Err(e) => {
                        let msg = format!("search object owner failed: {:?} {}", cur_owner_id, e);
                        warn!("{}", msg);

                        break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }
                }
            } else {
                if obj_type == ObjectTypeCode::Device {
                    // 本身就是device，那么直接返回
                    let device_id = DeviceId::try_from(object_id).unwrap();
                    Self::append_device_id(&mut device_list, device_id);

                    break Ok(());
                }

                let msg = format!("object owner not found : {}", object_id);
                warn!("{}", msg);

                break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        };

        match ret {
            Ok(_) => {
                if device_list.len() > 0 {
                    Ok(device_list)
                } else {
                    Err(BuckyError::from(BuckyErrorCode::NotFound))
                }
            }
            Err(e) => {
                if device_list.len() > 0 {
                    Ok(device_list)
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn get_ood_by_object_with_owner_impl(
        searcher: ObjectSearcherRef,
        mut object_id: ObjectId,
        mut owner_id: Option<ObjectId>,
    ) -> BuckyResult<Vec<DeviceId>> {
        let mut object: Option<Arc<AnyNamedObject>> = None;

        let mut device_list: Vec<DeviceId> = Vec::new();
        let ret = loop {
            let obj_type = object_id.obj_type_code();

            // People,SimpleGroup 对象存在ood_list
            if obj_type == ObjectTypeCode::People || obj_type == ObjectTypeCode::SimpleGroup {
                if object.is_some() {
                    match object.as_ref().unwrap().ood_list() {
                        Ok(list) => {
                            if list.len() > 0 {
                                debug!(
                                    "get ood list from object ood_list: {} {:?}",
                                    object_id, list
                                );
                                Self::append_device_list(&mut device_list, &list);
                                break Ok(());
                            } else {
                                let msg = format!(
                                    "get ood list from object ood_list empty: {}",
                                    object_id
                                );
                                warn!("{}", msg);
                                break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                            }
                        }
                        Err(e) => {
                            // 如果没指定ood_list，由于又不存在owner，终止查找
                            warn!("get ood_list from object failed! {} {}", object_id, e);
                            break Err(e);
                        }
                    }
                } else {
                    // 不能再search object本身，需要从owner出发
                }
            }

            // 其余类型，尝试从其owner出发继续查找
            match owner_id {
                Some(cur_owner_id) => {
                    // 如果owner就是device，那么返回这个device，不需要再继续查找
                    if cur_owner_id.obj_type_code() == ObjectTypeCode::Device {
                        info!("owner is device: obj={}, owner={}", object_id, cur_owner_id);

                        let device_id = DeviceId::try_from(cur_owner_id).unwrap();
                        Self::append_device_id(&mut device_list, device_id);

                        break Ok(());
                    }

                    // owner不能为自己
                    if object_id == cur_owner_id {
                        let msg = format!("object invalid owner: obj={}", object_id);
                        warn!("{}", msg);

                        break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                    }

                    // 从owner出发，再次查找对象
                    debug!(
                        "will search for owner: obj={}, owner={}",
                        object_id, cur_owner_id
                    );

                    match searcher.search(None, &cur_owner_id).await {
                        Ok(ret) => {
                            object_id = cur_owner_id;
                            object = ret.object;
                            owner_id = object.as_ref().unwrap().owner().clone();

                            continue;
                        }
                        Err(e) => {
                            let msg = format!("search object owner failed: {:?} {}", owner_id, e);
                            warn!("{}", msg);

                            break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                        }
                    }
                }
                None => {
                    if obj_type == ObjectTypeCode::Device {
                        // 本身就是device，那么直接返回
                        let device_id = DeviceId::try_from(object_id).unwrap();
                        Self::append_device_id(&mut device_list, device_id);

                        break Ok(());
                    }

                    let msg = format!("object not found or owner not specified: {}", object_id);
                    warn!("{}", msg);

                    break Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }
            }
        };

        match ret {
            Ok(_) => {
                assert!(device_list.len() > 0);
                Ok(device_list)
            }
            Err(e) => {
                if device_list.len() > 0 {
                    Ok(device_list)
                } else {
                    Err(e)
                }
            }
        }
    }
}