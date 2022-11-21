use cyfs_lib::*;
use cyfs_base::*;

use std::collections::HashMap;

pub(crate) struct DirHelper;

impl DirHelper {
    async fn get_value(noc: &NamedObjectCacheRef, source: &RequestSourceInfo, object_id: ObjectId) -> BuckyResult<Option<Vec<u8>>> {
        let resp = 
            noc
            .get_object(&NamedObjectCacheGetObjectRequest {
                source: source.to_owned(),
                object_id,
                last_access_rpath: None,
            })
            .await?;
            
        if resp.is_none() {
            Ok(None)
        } else {
            Ok(Some(resp.unwrap().object.object_raw))
        }
    }

    async fn put_value(noc: &NamedObjectCacheRef, source: &RequestSourceInfo, object_id: ObjectId, object_raw: Vec<u8>) -> BuckyResult<()> {
        let object = NONObjectInfo::new_from_object_raw(object_raw)?;

        let req = NamedObjectCachePutObjectRequest {
            source: source.to_owned(),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        noc.put_object(&req).await.map_err(|e| {
            error!("insert object map to noc error! id={}, {}", object_id, e);
            e
        })?;
        Ok(())
    }


    pub async fn build_zip_dir_from_object_map(source: &RequestSourceInfo, noc: &NamedObjectCacheRef, map_cache: ObjectMapOpEnvCacheRef, object_map_id: &ObjectId) -> BuckyResult<ObjectId> {
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
                        let file_raw = Self::get_value(noc, source, object_id.clone()).await?;
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

        Self::put_value(noc, source, dir_id.clone(), dir.to_vec()?).await?;

        Ok(dir_id)
    }
}
