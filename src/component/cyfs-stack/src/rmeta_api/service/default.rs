use super::super::service::GlobalStateMetaLocalService;
use cyfs_base::*;
use cyfs_lib::*;

pub(super) struct GlobalStateDefaultMetas {}

impl GlobalStateDefaultMetas {
    pub async fn init(local_service: &GlobalStateMetaLocalService) -> BuckyResult<()> {
        Self::init_access(local_service).await?;

        Ok(())
    }

    async fn init_access(local_service: &GlobalStateMetaLocalService) -> BuckyResult<()> {
        let root_state_meta = local_service.get_meta_manager(GlobalStateCategory::RootState);
        let meta = root_state_meta
            .get_global_state_meta(cyfs_core::get_system_dec_app(), true)
            .await?;

        // admin manager call
        let item = GlobalStatePathAccessItem {
            path: CYFS_SYSTEM_ADMIN_VIRTUAL_PATH.to_owned(),
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone: None,
                zone_category: Some(DeviceZoneCategory::CurrentZone),
                dec: Some(cyfs_core::get_system_dec_app().to_owned()),
                access: AccessPermissions::CallOnly as u8,
            }),
        };

        meta.add_access(item).await?;

        // app manager call
        let item = GlobalStatePathAccessItem {
            path: CYFS_SYSTEM_APP_VIRTUAL_PATH.to_owned(),
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone: None,
                zone_category: Some(DeviceZoneCategory::CurrentDevice),
                dec: Some(cyfs_core::get_system_dec_app().to_owned()),
                access: AccessPermissions::CallOnly as u8,
            }),
        };

        meta.add_access(item).await?;

        // role manager call
        let item = GlobalStatePathAccessItem {
            path: CYFS_SYSTEM_ROLE_VIRTUAL_PATH.to_owned(),
            access: GlobalStatePathGroupAccess::Specified(GlobalStatePathSpecifiedGroup {
                zone: None,
                zone_category: Some(DeviceZoneCategory::CurrentZone),
                dec: Some(cyfs_core::get_system_dec_app().to_owned()),
                access: AccessPermissions::CallOnly as u8,
            }),
        };

        meta.add_access(item).await?;

        // allow crypto.verify_object for all zone dec call
        let mut permissions = AccessString::new(0);
        permissions.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
        permissions.set_group_permission(AccessGroup::CurrentDevice, AccessPermission::Call);
        permissions.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
        permissions.set_group_permission(AccessGroup::OwnerDec, AccessPermission::Call);

        let path = format!("{}/{}", CYFS_CRYPTO_VIRTUAL_PATH, "verify_object");
        let item = GlobalStatePathAccessItem {
            path,
            access: GlobalStatePathGroupAccess::Default(permissions.value()),
        };

        meta.add_access(item).await?;

        info!("init defualt rmeta access success!");

        Ok(())
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {}
}
