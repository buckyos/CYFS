use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct ObjectMapNOCCacheAdapter {
    noc: Box<dyn NamedObjectCache>,
    device_id: DeviceId,
}

impl ObjectMapNOCCacheAdapter {
    pub fn new(device_id: &DeviceId, noc: Box<dyn NamedObjectCache>) -> Self {
        Self {
            device_id: device_id.to_owned(),
            noc,
        }
    }

    pub fn new_noc_cache(
        device_id: &DeviceId,
        noc: Box<dyn NamedObjectCache>,
    ) -> ObjectMapNOCCacheRef {
        let ret = Self::new(device_id, noc);
        Arc::new(Box::new(ret) as Box<dyn ObjectMapNOCCache>)
    }
}

#[async_trait::async_trait]
impl ObjectMapNOCCache for ObjectMapNOCCacheAdapter {
    async fn exists(&self, object_id: &ObjectId) -> BuckyResult<bool> {
        // TODO noc支持exists方法
        
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: NONProtocol::Native,
            object_id: object_id.clone(),
            source: self.device_id.clone(),
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object map from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(_) => {
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn get_object_map(&self, object_id: &ObjectId) -> BuckyResult<Option<ObjectMap>> {
        let noc_req = NamedObjectCacheGetObjectRequest {
            protocol: NONProtocol::Native,
            object_id: object_id.clone(),
            source: self.device_id.clone(),
        };

        let resp = self.noc.get_object(&noc_req).await.map_err(|e| {
            error!("load object map from noc error! id={}, {}", object_id, e);
            e
        })?;

        match resp {
            Some(resp) => {
                assert!(resp.object.is_some());
                assert!(resp.object_raw.is_some());
                let buf = resp.object_raw.unwrap();
                match ObjectMap::raw_decode(&buf) {
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
            None => Ok(None),
        }
    }

    async fn put_object_map(&self, object_id: ObjectId, object: ObjectMap) -> BuckyResult<()> {
        let dec_id = object.desc().dec_id().to_owned();
        let object_raw = object.to_vec().unwrap();
        let object = AnyNamedObject::Standard(StandardObject::ObjectMap(object));
        // let (object, _) = AnyNamedObject::raw_decode(&object_raw).unwrap();

        let req = NamedObjectCacheInsertObjectRequest {
            protocol: NONProtocol::Native,
            source: self.device_id.clone(),
            object_id: object_id,
            dec_id,
            object_raw,
            object: Arc::new(object),
            flags: 0u32,
        };

        self.noc.insert_object(&req).await.map_err(|e| {
            error!(
                "insert object map to noc error! id={}, dec={:?}, {}",
                object_id, dec_id, e
            );
            e
        })?;

        Ok(())
    }
}
