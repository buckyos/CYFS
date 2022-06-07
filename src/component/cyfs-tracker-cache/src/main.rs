mod sqlite;
mod tracker_cache_manager;

#[macro_use]
extern crate log;

pub use tracker_cache_manager::*;

#[async_std::main]
async fn main() {
    use cyfs_base::*;
    use cyfs_lib::*;
    use std::str::FromStr;

    cyfs_util::init_log("cyfs-tracker-cache", Some("trace"));

    let cache = TrackerCacheManager::create_tracker_cache("test").unwrap();

    let pos_file = PostionFileRange {
        path: "xxxxxx:xxxx".to_owned(),
        range_begin: 1000,
        range_end: 2000,
    };
    let pos = TrackerPostion::FileRange(pos_file);

    let req = AddTrackerPositonRequest {
        id: "test-object".to_owned(),
        direction: TrackerDirection::Store,
        pos,
        flags: 0x1111,
    };

    if let Err(e) = cache.add_position(&req).await {
        error!("add pos error: {}", e);
    } else {
        info!("add pos success!");
    }

    let device_id = DeviceId::from_str("5bnZHzY5D258zsLeSAYikoSrUh51qRDFwaVBNDme1G2D").unwrap();
    let req2 = AddTrackerPositonRequest {
        id: "test-object".to_owned(),
        direction: TrackerDirection::To,
        pos: TrackerPostion::Device(device_id),
        flags: 0x2222,
    };

    if let Err(e) = cache.add_position(&req2).await {
        error!("add pos2 error: {}", e);
    } else {
        info!("add pos2 success!");
    }

    {
        let get_req = GetTrackerPositionRequest {
            id: req.id.clone(),
            direction: None,
        };

        let list = cache.get_position(&get_req).await.unwrap();
        assert!(list.len() == 2);
        info!("get pos item1: {:?}", list[0]);
        info!("get pos item2: {:?}", list[1]);
    }

    {
        let get_req = GetTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(TrackerDirection::From),
        };

        let list = cache.get_position(&get_req).await.unwrap();
        assert!(list.len() == 0);
    }

    {
        let get_req = GetTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(req.direction.clone()),
        };

        let mut list = cache.get_position(&get_req).await.unwrap();
        assert!(list.len() == 1);
        info!("get pos item: {:?}", list[0]);
        let get_item = list.pop().unwrap();
        assert!(get_item.direction == req.direction);
        assert!(get_item.flags == req.flags);
        assert!(get_item.pos == req.pos);
    }

    {
        let get_req = GetTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(TrackerDirection::To),
        };

        let mut list = cache.get_position(&get_req).await.unwrap();
        assert!(list.len() == 1);
        info!("get pos item: {:?}", list[0]);
        let get_item = list.pop().unwrap();
        assert!(get_item.direction == req2.direction);
        assert!(get_item.flags == req2.flags);
        assert!(get_item.pos == req2.pos);
    }

    // 测试删除所有
    if false
    {
        let remove_req = RemoveTrackerPositionRequest {
            id: req.id.clone(),
            direction: None,
            pos: None,
        };

        let count = cache.remove_position(&remove_req).await.unwrap();
        assert!(count == 2);
    }

    {
        let remove_req = RemoveTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(TrackerDirection::From),
            pos: None,
        };

        let count = cache.remove_position(&remove_req).await.unwrap();
        assert!(count == 0);

        let remove_req = RemoveTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(TrackerDirection::To),
            pos: Some(req2.pos.clone()),
        };

        let count = cache.remove_position(&remove_req).await.unwrap();
        assert!(count == 1);

        let get_req = GetTrackerPositionRequest {
            id: req.id.clone(),
            direction: Some(TrackerDirection::To),
        };

        let list = cache.get_position(&get_req).await.unwrap();
        assert!(list.len() == 0);
    }
}
