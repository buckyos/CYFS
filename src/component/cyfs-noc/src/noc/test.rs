use cyfs_lib::NONObjectInfo;
use cyfs_core::*;
use cyfs_base::*;
use crate::*;


fn new_object(id: &str) -> NONObjectInfo {
    let obj = Text::create(id, "", "");
    NONObjectInfo::new_from_object_raw(obj.to_vec().unwrap()).unwrap()
}

fn new_dec(name: &str) -> ObjectId {
    let owner_id = PeopleId::default();
    let dec_id = DecApp::generate_id(owner_id.into(), name);

    info!(
        "generage random name={}, dec_id={}",
        name, dec_id
    );

    dec_id
}

async fn test_noc() {
    cyfs_base::init_simple_log("cyfs-noc-test", Some("debug"));

    let noc = NamedObjectCacheManager::create("test").await.unwrap();

    let object = new_object("test-local");
    let update_time = object.object.as_ref().unwrap().update_time().unwrap();

    let object_id = object.object_id.clone();
    let put_req = NamedObjectCachePutObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object: object,
        storage_category: NamedObjectStorageCategory::Storage,
        context: None,
        last_access_rpath: None,
        access_string: None,
    };

    noc.put_object(&put_req).await.unwrap();

    // put by other dec
    let dec1 = new_dec("dec1");
    let source = RequestSourceInfo {
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        },
        dec: dec1,
    };

    let object = new_object("test-local");
    let object_id = object.object_id.clone();
    let put_req = NamedObjectCachePutObjectRequest {
        source,
        object: object,
        storage_category: NamedObjectStorageCategory::Storage,
        context: None,
        last_access_rpath: None,
        access_string: None,
    };

    if let Err(e) = noc.put_object(&put_req).await {
        assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
    } else {
        unreachable!();
    }
    

    // get by system
    let get_req = NamedObjectCacheGetObjectRequest1 {
        source: RequestSourceInfo::new_local_system(),
        object_id: object_id.to_owned(),
        last_access_rpath: None,
    };

    let ret = noc.get_object(&get_req).await.unwrap();
    assert!(ret.is_some());
    let data = ret.unwrap();
    let got_update_time = data.object.as_ref().unwrap().object.as_ref().unwrap().update_time().unwrap();
    assert_eq!(got_update_time, update_time);

    // get by unknown device
    let source = RequestSourceInfo {
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::OtherZone,
        },
        dec: cyfs_core::get_system_dec_app().object_id().to_owned(),
    };

    let get_req = NamedObjectCacheGetObjectRequest1 {
        source,
        object_id: object_id.to_owned(),
        last_access_rpath: None,
    };

    if let Err(e) = noc.get_object(&get_req).await {
        assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
    } else {
        unreachable!();
    }

    // get by other dec
    let dec1 = new_dec("dec1");
    let source = RequestSourceInfo {
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        },
        dec: dec1,
    };
    let get_req = NamedObjectCacheGetObjectRequest1 {
        source,
        object_id: object_id.to_owned(),
        last_access_rpath: None,
    };

    let ret = noc.get_object(&get_req).await;
    match ret {
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        }
        Ok(_) => {
            unreachable!();
        }
    }
}

#[test]
fn main() {
    async_std::task::block_on(async move {
        test_noc().await;
    });
}
