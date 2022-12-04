use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, OOD_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct SameZoneRmetaBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}

pub const RMETA_INNER_ZONE_ACCESS: &str = "rmeta-inner-zone-access";
const LIST: [&str;1] = [
    RMETA_INNER_ZONE_ACCESS,
];

#[async_trait]
impl Bench for SameZoneRmetaBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
        
    }

    fn name(&self) -> &str {
        "Same Zone rmeta Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl SameZoneRmetaBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            self.test_rmeta(i).await?;

            self.stat.write(self.name(),RMETA_INNER_ZONE_ACCESS, begin.elapsed().as_millis() as u64);
        }

        Ok(())
    }

    async fn test_rmeta(&self, _i: usize) -> BuckyResult<()> {
        info!("begin test_rmeta_access...");    
        // {
        //     let meta =
        //     self.stack.root_state_meta_stub(None, Some(cyfs_core::get_system_dec_app().to_owned()));
    
        //     let access = AccessString::dec_default();
        //     let item = GlobalStatePathAccessItem {
        //         path: "/a/b".to_owned(),
        //         access: GlobalStatePathGroupAccess::Default(access.value()),
        //     };
    
        //     if let Err(e) = meta.add_access(item).await {
        //         assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        //     } else {
        //         unreachable!();
        //     }
        // }
    
        self.stack
        .root_state_meta_stub(None, None)
        .clear_access()
        .await
        .unwrap();

        let meta = self.stack.root_state_meta_stub(None, None);
    
        let access = AccessString::dec_default();
        let item = GlobalStatePathAccessItem {
            path: "/a/b".to_owned(),
            access: GlobalStatePathGroupAccess::Default(access.value()),
        };
    
        meta.add_access(item).await.unwrap();
    
        let perm = AccessPermissions::ReadAndCall as u8;
        let item = GlobalStatePathAccessItem {
            path: "/a/b".to_owned(),
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone: None,
                zone_category: Some(DeviceZoneCategory::CurrentZone),
                dec: Some(OOD_DEC_ID.clone()),
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
                dec: Some(OOD_DEC_ID.clone()),
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
                dec: Some(OOD_DEC_ID.clone()),
                access: 0,
            }),
        };
    
        let ret = meta.remove_access(item).await.unwrap();
        assert!(ret.is_some());
        let current = ret.unwrap();
        assert_eq!(current.path, "/a/b/");
        if let GlobalStatePathGroupAccess::Specified(value) = current.access {
            assert_eq!(value.access, perm);
            assert_eq!(value.dec, Some(OOD_DEC_ID.clone()));
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
    
        Ok(())
    }
}