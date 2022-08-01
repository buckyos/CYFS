mod named_object_storage;
mod object_cache_manager;
mod common;
mod blob;

#[cfg(feature = "mongo")]
mod mongodb;

#[cfg(feature = "memory")]
mod memory;

#[cfg(feature = "sqlite")]
mod sqlite;


pub use object_cache_manager::*;
pub use named_object_storage::*;

#[macro_use]
extern crate log;


#[async_std::test]
async fn test() {
    /*
    cyfs_debug::init_log("cyfs-noc", None);

    OBJECT_CACHE_MANAGER
        .lock()
        .unwrap()
        .init(NamedObjectStorageType::Sqlite)
        .await
        .unwrap();

    let object_manager = OBJECT_CACHE_MANAGER.lock().unwrap().clone();

    let device_info = LOCAL_DEVICE&_MANAGER.load("device").unwrap();
    let device = &device_info.device;

    let device_id = device.desc().device_id();

    let mut req = ObjectRequest::new(
        NONProtocol::HttpLocal,
        device_id.clone(),
        device_id.object_id().clone(),
        None,
        None,
        None,
        0,
    );

    req.bind_object(device.to_vec().unwrap()).unwrap();

    object_manager.insert_object(&req).await.unwrap();

    let cur_req = object_manager
        .get_object(device_id.object_id())
        .await
        .unwrap();
    match cur_req {
        Some(cur_req) => {
            info!("query object success!");
            assert_eq!(cur_req.object_id, *device_id.object_id());
            assert_eq!(cur_req.device_id, device_id);
            assert_eq!(cur_req.object_raw, req.object_raw);
        }
        None => {
            info!("object not exists, now will insert");
            object_manager.insert_object(&req).await.unwrap();
        }
    }

    let mut filter = NamedObjectCacheSelectObjectFilter::default();
    filter.update_time = Some(NamedObjectCacheSelectObjectTimeRange {
        begin: Some(13244722316339283_u64),
        end: None,
    });

    filter.insert_time = Some(NamedObjectCacheSelectObjectTimeRange {
        begin: Some(13245249753119327_u64),
        end: Some(13245249753119327_u64),
    });

    let opt = NamedObjectCacheSelectObjectOption::default();

    match object_manager.select_object(&filter, Some(opt)).await {
        Ok(resp) => {
            info!("select object success! count={}", resp.len());
            for item in resp {
                info!(
                    "object={}, insert_time={} update_time={}",
                    item.object.as_ref().unwrap().calculate_id(),
                    item.insert_time,
                    item.update_time
                );
            }
        }
        Err(e) => {
            error!("select object failed! {}", e);
        }
    }
     */
}