use crate::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

fn new_object(id: &str) -> NONObjectInfo {
    let obj = Text::create(id, "", "");
    NONObjectInfo::new_from_object_raw(obj.to_vec().unwrap()).unwrap()
}

fn new_dec(name: &str) -> ObjectId {
    let owner_id = PeopleId::default();
    let dec_id = DecApp::generate_id(owner_id.into(), name);

    info!("generage random name={}, dec_id={}", name, dec_id);

    dec_id
}

async fn test_noc() {
    cyfs_base::init_simple_log("cyfs-noc-test", Some("debug"));

    let noc = NamedObjectCacheManager::create("test").await.unwrap();

    let object = new_object("test-local");
    let update_time = object.object.as_ref().unwrap().update_time().unwrap();

    let mut access = AccessString::new(0);
    access.set_group_permissions(AccessGroup::CurrentDevice, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::CurrentZone, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::OwnerDec, AccessPermissions::Full);
    let put_req = NamedObjectCachePutObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object: object,
        storage_category: NamedObjectStorageCategory::Storage,
        context: None,
        last_access_rpath: None,
        access_string: Some(access.value()),
    };

    noc.put_object(&put_req).await.unwrap();

    // put by other dec
    let dec1 = new_dec("dec1");
    let source = RequestSourceInfo {
        protocol: RequestProtocol::Native,
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        },
        dec: dec1,
        verified: None,
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

    // exists
    let req = NamedObjectCacheExistsObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: object_id.clone(),
    };

    let ret = noc.exists_object(&req).await.unwrap();
    assert!(ret.meta);
    assert!(ret.object);

    let req = NamedObjectCacheExistsObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: ObjectId::default(),
    };

    let ret = noc.exists_object(&req).await.unwrap();
    assert!(!ret.meta);
    assert!(!ret.object);

    // get by system
    let get_req = NamedObjectCacheGetObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: object_id.to_owned(),
        last_access_rpath: Some("/test/dec1".to_owned()),
    };

    let ret = noc.get_object(&get_req).await.unwrap();
    assert!(ret.is_some());
    let data = ret.unwrap();
    let got_update_time = data.object.object.as_ref().unwrap().update_time().unwrap();
    assert_eq!(got_update_time, update_time);

    // update meta
    let mut access = AccessString::new(0);
    access.set_group_permissions(AccessGroup::CurrentDevice, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::CurrentZone, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::FriendZone, AccessPermissions::Full);
    access.set_group_permissions(AccessGroup::OwnerDec, AccessPermissions::Full);
    let context = "test1".to_owned();
    let last_access_rpath = "/test/last_access_rpath".to_owned();
    let put_req = NamedObjectCacheUpdateObjectMetaRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: data.object.object_id.clone(),
        storage_category: Some(NamedObjectStorageCategory::Cache),
        context: Some(context.clone()),
        last_access_rpath: Some(last_access_rpath.clone()),
        access_string: Some(access.value()),
    };

    noc.update_object_meta(&put_req).await.unwrap();

    // reget
    let get_req = NamedObjectCacheGetObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: object_id.to_owned(),
        last_access_rpath: None,
    };

    let ret = noc.get_object(&get_req).await.unwrap();
    assert!(ret.is_some());
    let data = ret.unwrap();
    assert_eq!(*data.meta.context.as_ref().unwrap(), context);
    assert_eq!(
        *data.meta.last_access_rpath.as_ref().unwrap(),
        last_access_rpath
    );
    assert_eq!(
        data.meta.storage_category,
        NamedObjectStorageCategory::Cache
    );
    assert_eq!(data.meta.access_string, access.value());

    // get by unknown device
    let source = RequestSourceInfo {
        protocol: RequestProtocol::Native,
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::OtherZone,
        },
        dec: cyfs_core::get_system_dec_app().to_owned(),
        verified: None,
    };

    let get_req = NamedObjectCacheGetObjectRequest {
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
    let dec2 = new_dec("dec1");
    let source = RequestSourceInfo {
        protocol: RequestProtocol::Native,
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        },
        dec: dec2.clone(),
        verified: None,
    };
    let get_req = NamedObjectCacheGetObjectRequest {
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

    // select
    let select_req = NamedObjectCacheSelectObjectRequest {
        filter: NamedObjectCacheSelectObjectFilter { obj_type: None },
        opt: NamedObjectCacheSelectObjectOption::default(),
    };

    let resp = noc.select_object(&select_req).await.unwrap();
    info!("select result: {:?}", resp);

    // delete by system
    let delete_req = NamedObjectCacheDeleteObjectRequest {
        source: RequestSourceInfo::new_local_system(),
        object_id: object_id.to_owned(),
        flags: CYFS_NOC_FLAG_DELETE_WITH_QUERY,
    };

    let ret = noc.delete_object(&delete_req).await.unwrap();
    assert_eq!(ret.deleted_count, 1);
    let got_update_time = ret
        .object
        .as_ref()
        .unwrap()
        .object
        .as_ref()
        .unwrap()
        .update_time()
        .unwrap();
    assert_eq!(got_update_time, update_time);

    // select
    let select_req = NamedObjectCacheSelectObjectRequest {
        filter: NamedObjectCacheSelectObjectFilter { obj_type: None },
        opt: NamedObjectCacheSelectObjectOption::default(),
    };

    let resp = noc.select_object(&select_req).await.unwrap();
    info!("select result: {:?}", resp);
}

async fn test_error_blob() {
    use std::str::FromStr;

    cyfs_base::init_simple_log("cyfs-noc-test", Some("debug"));

    let noc = NamedObjectCacheManager::create("error").await.unwrap();

    let object_id = ObjectId::from_str("9cfBkPtFSnksaLsAHpDXtquYG46TRj1xHLsqqM9jFagi").unwrap();
    info!("object={}, {}", object_id, object_id.to_base36());

    let dec2 = new_dec("dec1");
    let source = RequestSourceInfo {
        protocol: RequestProtocol::Native,
        zone: DeviceZoneInfo {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        },
        dec: dec2.clone(),
        verified: None,
    };
    let get_req = NamedObjectCacheGetObjectRequest {
        source,
        object_id: object_id.clone(),
        last_access_rpath: None,
    };

    let resp = noc.get_object(&get_req).await.unwrap();
    let data = resp.unwrap();
    let _data = Storage::raw_decode(&data.object.object_raw).unwrap();
    info!("test complete!");
}

#[test]
fn main() {
    async_std::task::block_on(async move {
        // test_error_blob().await;
        test_noc().await;
    });
}
