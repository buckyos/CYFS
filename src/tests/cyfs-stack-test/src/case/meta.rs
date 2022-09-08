use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;


fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, owner_id
    );

    dec_id
}

pub async fn test() {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let device_stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device2_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);

    test_meta(&device_stack).await;
}

async fn test_meta(stack: &SharedCyfsStack) {
    let meta = stack.root_state_meta_stub(None, None);

    let access = AccessString::dec_default();
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };

    meta.add_access(item).await.unwrap();

    let perm = AccessPermissions::ReadAndCall as u8;
    let other_dec = new_dec("other1");
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            dec: Some(other_dec.clone()),
            access: perm,
        }),
    };

    meta.add_access(item).await.unwrap();

    // test remove
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            dec: Some(other_dec.clone()),
            access: 0,
        }),
    };

    let ret = meta.remove_access(item).await.unwrap();
    assert!(ret.is_some());
    let current = ret.unwrap();
    assert_eq!(current.path, "/a/b");
    if let GlobalStatePathGroupAccess::Specified(value) =  current.access {
        assert_eq!(value.access, perm);
        assert_eq!(value.dec, Some(other_dec.clone()));
        assert!(value.zone.is_none());
    } else {
        unreachable!();
    }

    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Default(0),
    };

    let ret = meta.remove_access(item).await.unwrap();
    assert!(ret.is_some());
    let current = ret.unwrap();
    assert_eq!(current.path, "/a/b");

    if let GlobalStatePathGroupAccess::Default(value) =  current.access {
        assert_eq!(value, access.value());
    } else {
        unreachable!();
    }

    // clear
    let count = meta.clear_access().await.unwrap();
    assert_eq!(count, 0);
}
