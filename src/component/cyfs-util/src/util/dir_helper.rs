use crate::*;
use cyfs_base::*;

use std::collections::HashMap;
use std::sync::Arc;

pub struct DirHelper;

#[async_trait::async_trait]
pub trait ObjectCache: Send + Sync {
    async fn get_value(&self, object_id: ObjectId) -> BuckyResult<Option<Vec<u8>>>;
    async fn put_value(&self, object_id: ObjectId, object_raw: Vec<u8>) -> BuckyResult<()>;
    async fn is_exist(&self, object_id: ObjectId) -> BuckyResult<bool>;
}
pub type ObjectCacheRef = Arc<dyn ObjectCache>;

struct ObjectMapCache {
    object_cache: ObjectCacheRef,
}

impl ObjectMapCache {
    pub fn new(object_cache: ObjectCacheRef) -> Self {
        Self {
            object_cache,
        }
    }

    pub fn new_noc_cache(
        object_cache: ObjectCacheRef,
    ) -> ObjectMapNOCCacheRef {
        let ret = Self::new(object_cache);
        Arc::new(Box::new(ret) as Box<dyn ObjectMapNOCCache>)
    }
}

#[async_trait::async_trait]
impl ObjectMapNOCCache for ObjectMapCache {
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        match self.object_cache.get_value(object_id.clone()).await {
            Ok(_) => {
                Ok(true)
            },
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    Ok(false)
                } else {
                    error!("load object map from noc error! id={}, {}", object_id, e);
                    Err(e)
                }
            }
        }
    }

    async fn get_object_map(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectMap>> {
        let resp = self.object_cache.get_value(object_id.clone()).await.map_err(|e| {
            error!("load object map from noc error! id={}, {}", object_id, e);
            e
        })?;

        if resp.is_none() {
            return Ok(None);
        }

        match ObjectMap::raw_decode(resp.unwrap().as_slice()) {
            Ok((obj, _)) => {
                // 首次加载后，直接设置id缓存，减少一次id计算
                obj.direct_set_object_id_on_init(object_id);

                Ok(Some(obj))
            }
            Err(e) => {
                error!("decode ObjectMap object error: id={}, {}", object_id, e);
                Err(e)
            }
        }
    }

    async fn put_object_map(&self, object_id: ObjectId, object: ObjectMap) -> BuckyResult<()> {
        let object_raw = object.to_vec().unwrap();
        self.object_cache.put_value(object_id, object_raw).await.map_err(|e| {
            error!(
                "insert object map to noc error! id={}, {}",
                object_id, e
            );
            e
        })?;

        Ok(())
    }
}

fn new_object_map_cache(object_cache: ObjectCacheRef) -> ObjectMapOpEnvCacheRef {
    let noc = ObjectMapCache::new_noc_cache(object_cache);
    let root_cache = ObjectMapRootMemoryCache::new_default_ref(noc);
    let cache = ObjectMapOpEnvMemoryCache::new_ref(root_cache.clone());
    cache
}

impl DirHelper {
    pub async fn build_zip_dir_from_object_map(object_cache: ObjectCacheRef, object_map_id: &ObjectId) -> BuckyResult<ObjectId> {
        let map_cache = new_object_map_cache(object_cache.clone());
        let root_map = map_cache.get_object_map(object_map_id).await?;
        if root_map.is_none() {
            let msg = format!("object map {} is not exist", object_map_id.to_string());
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let owner = root_map.as_ref().unwrap().lock().await.desc().owner().clone();

        let mut bodys = HashMap::new();
        let mut entrys = HashMap::new();

        let mut it = ObjectMapPathIterator::new(
            root_map.unwrap(),
            map_cache,
            ObjectMapPathIteratorOption::new(true, false)).await;
        while !it.is_end() {
            let list = it.next(10).await?;
            for item in list.list.into_iter() {
                if let ObjectMapContentItem::Map((name, object_id)) = item.value {
                    if object_id.obj_type_code() == ObjectTypeCode::File {
                        let file_raw = object_cache.get_value(object_id.clone()).await?;
                        if file_raw.is_none() {
                            let msg = format!("file object {} is not exist", object_id.to_string());
                            log::error!("{}", msg.as_str());
                            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                        } else {
                            bodys.insert(object_id.clone(), file_raw.unwrap());

                            let inner_file_path = format!("{}/{}", item.path, name);
                            #[cfg(target_os = "windows")]
                            let inner_file_path = inner_file_path.replace("\\", "/");
                            // 内部路径不能以/开头
                            let inner_file_path = inner_file_path.trim_start_matches('/').to_owned();

                            entrys.insert(inner_file_path,
                                          InnerNodeInfo::new(
                                              Attributes::new(0),
                                              InnerNode::ObjId(object_id)));
                        }
                    } else {
                        let msg = format!("object {} is not file", object_id.to_string());
                        log::error!("{}", msg.as_str());
                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }
                } else {
                    let msg = format!("invalid object map type");
                    log::error!("{}", msg.as_str());
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            }
        }

        let dir = if owner.is_none() {
            Dir::new(
                Attributes::new(0),
                NDNObjectInfo::ObjList(NDNObjectList {
                    parent_chunk: None,
                    object_map: entrys,
                }),
                bodys,
            )
                .create_time(0)
                .build()

        } else {
            Dir::new(
                Attributes::new(0),
                NDNObjectInfo::ObjList(NDNObjectList {
                    parent_chunk: None,
                    object_map: entrys,
                }),
                bodys,
            )
                .create_time(0)
                .owner(owner.unwrap())
                .build()
        };

        let dir_id = dir.desc().calculate_id();

        object_cache.put_value(dir_id.clone(), dir.to_vec()?).await?;

        Ok(dir_id)
    }
}
