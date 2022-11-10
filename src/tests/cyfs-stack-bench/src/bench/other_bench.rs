use async_trait::async_trait;
use cyfs_base::{AccessString, BuckyErrorCode, AccessPermissions, ObjectId};
use cyfs_core::{DecApp, DecAppObj};
use cyfs_lib::{GlobalStatePathAccessItem, GlobalStatePathGroupAccess, GlobalStatePathSpecifiedGroup, DeviceZoneCategory};
use crate::{Bench, BenchEnv, sim_zone::SimZone, bench::RMETA_INNER_ZONE_ACCESS, stat::Stat};
use log::*;

pub struct OtherBench {}

#[async_trait]
impl Bench for OtherBench {
    async fn bench(&self, env: BenchEnv, zone: &SimZone, _ood_path: String, t: u64) -> bool {
        info!("begin test OtherBench...");
        let begin = std::time::Instant::now();

        let ret = if env == BenchEnv::Simulator {
            for _ in 0..t {
                let _ret = test(zone).await;
            }

            true
        } else {
            // TODO: support physical stack  ood/runtime
            true
        };

        let dur = begin.elapsed();
        info!("end test OtherBench: {:?}", dur);

        ret
        
    }

    fn name(&self) -> &str {
        "Other Bench"
    }
}


fn new_dec(name: &str, zone: &SimZone) -> ObjectId {
    let people_id = zone.get_object_id_by_name("zone1_people");

    let dec_id = DecApp::generate_id(people_id, name);

    info!(
        "generage test storage dec_id={}, people={}",
        dec_id, people_id
    );

    dec_id
}

pub async fn test(zone: &SimZone) -> bool {
    info!("begin test_rmeta_access...");
    let begin = std::time::Instant::now();

    let dec1 = new_dec("User1Device1.rmeta", zone);
    let device1 = zone.get_shared_stack("zone1_device1")
        .fork_with_new_dec(Some(dec1.clone()))
        .await
        .unwrap();
    device1.wait_online(None).await.unwrap();

    {
        let meta =
        device1.root_state_meta_stub(None, Some(cyfs_core::get_system_dec_app().to_owned()));

        let access = AccessString::dec_default();
        let item = GlobalStatePathAccessItem {
            path: "/a/b".to_owned(),
            access: GlobalStatePathGroupAccess::Default(access.value()),
        };

        if let Err(e) = meta.add_access(item).await {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        } else {
            unreachable!();
        }
    }

    let meta = device1.root_state_meta_stub(None, None);

    let access = AccessString::dec_default();
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Default(access.value()),
    };

    meta.add_access(item).await.unwrap();

    let perm = AccessPermissions::ReadAndCall as u8;
    let other_dec = new_dec("other1", zone);
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(other_dec.clone()),
            access: perm,
        }),
    };

    meta.add_access(item).await.unwrap();

    // test error remove
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: None,
            dec: Some(other_dec.clone()),
            access: 0,
        }),
    };

    let ret = meta.remove_access(item).await.unwrap();
    assert!(ret.is_none());

    // test remove
    let item = GlobalStatePathAccessItem {
        path: "/a/b".to_owned(),
        access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
            zone: None,
            zone_category: Some(DeviceZoneCategory::CurrentZone),
            dec: Some(other_dec.clone()),
            access: 0,
        }),
    };

    let ret = meta.remove_access(item).await.unwrap();
    assert!(ret.is_some());
    let current = ret.unwrap();
    assert_eq!(current.path, "/a/b/");
    if let GlobalStatePathGroupAccess::Specified(value) = current.access {
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
    assert_eq!(current.path, "/a/b/");

    if let GlobalStatePathGroupAccess::Default(value) = current.access {
        assert_eq!(value, access.value());
    } else {
        unreachable!();
    }

    let dur = begin.elapsed();
    info!("end test_rmeta_access: {:?}", dur);
    let costs = begin.elapsed().as_millis() as u64;
    Stat::write(zone, RMETA_INNER_ZONE_ACCESS, costs).await;

    true
}