use super::sqlite::*;
use cyfs_base::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;

enum CheckAccessData<'a> {
    MetaData(&'a NamedObjectMetaData),
    MetaUpdateInfo(&'a NamedObjectMetaUpdateInfo),
}

pub(crate) struct NamedObjecAccessHelper {
    object_meta_access_provider: OnceCell<NamedObjectCacheObjectMetaAccessProviderRef>,
}

impl NamedObjecAccessHelper {
    pub fn new() -> Self {
        Self {
            object_meta_access_provider: OnceCell::new(),
        }
    }

    pub fn bind_object_meta_access_provider(
        &self,
        object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        if let Err(_) = self
            .object_meta_access_provider
            .set(object_meta_access_provider)
        {
            unreachable!();
        }
    }

    fn check_object_access(
        object_id: &ObjectId,
        access_string: u32,
        source: &RequestSourceInfo,
        create_dec_id: &ObjectId,
        permissions: impl Into<AccessPermissions>,
    ) -> BuckyResult<()> {
        let permissions: AccessPermissions = permissions.into();
        debug!("noc meta will check access: object={}, access={}, source={}, create_dec={}, require={}", 
            object_id, AccessString::new(access_string), source, create_dec_id, permissions.as_str());

        // Check permission first
        let mask = source.mask(create_dec_id, permissions);

        if access_string & mask != mask {
            let msg = format!(
                "noc meta object access been rejected! obj={}, access={}, require access={}",
                object_id,
                AccessString::new(access_string),
                AccessString::new(mask)
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }

    pub async fn check_access_with_meta_data(
        &self,
        object_id: &ObjectId,
        source: &RequestSourceInfo,
        data: &NamedObjectMetaData,
        create_dec_id: &ObjectId,
        permissions: impl Into<AccessPermissions>,
    ) -> BuckyResult<()> {
        let data = CheckAccessData::MetaData(data);
        self.check_access(object_id, source, data, create_dec_id, permissions)
            .await
    }

    pub async fn check_access_with_meta_update_info(
        &self,
        object_id: &ObjectId,
        source: &RequestSourceInfo,
        data: &NamedObjectMetaUpdateInfo,
        create_dec_id: &ObjectId,
        permissions: impl Into<AccessPermissions>,
    ) -> BuckyResult<()> {
        let data = CheckAccessData::MetaUpdateInfo(data);
        self.check_access(object_id, source, data, create_dec_id, permissions)
            .await
    }

    async fn check_access<'a>(
        &self,
        object_id: &ObjectId,
        source: &RequestSourceInfo,
        data: CheckAccessData<'a>,
        create_dec_id: &ObjectId,
        permissions: impl Into<AccessPermissions>,
    ) -> BuckyResult<()> {
        if source.is_verified(&create_dec_id) {
            return Ok(());
        }

        // system dec or same dec in current zone is always allowed
        if source.is_current_zone() {
            if source.check_target_dec_permission2(Some(create_dec_id)) {
                return Ok(());
            }
        }

        let permissions = permissions.into();
        if let Some(object_meta) = self.object_meta_access_provider.get() {
            debug!("noc meta will check object meta access: object={}, source={}, create_dec={}, require={}", 
                object_id, source, create_dec_id, permissions);

            let ret = match data {
                CheckAccessData::MetaData(data) => {
                    object_meta
                        .check_access(&create_dec_id, data, &source, permissions)
                        .await?
                }
                CheckAccessData::MetaUpdateInfo(info) => {
                    let data = NamedObjectMetaUpdateInfoDataProvider {
                        info: &info,
                        object_id: &object_id,
                    };
                    object_meta
                        .check_access(&create_dec_id, &data, &source, permissions)
                        .await?
                }
            };

            if ret.is_some() {
                return Ok(());
            }
        } else {
            warn!(
                "check object access but object meta provider has not been inited yet! id={}",
                object_id
            );
        }

        let access_string = match data {
            CheckAccessData::MetaData(data) => data.access_string,
            CheckAccessData::MetaUpdateInfo(info) => info.access_string,
        };

        Self::check_object_access(
            &object_id,
            access_string,
            &source,
            &create_dec_id,
            permissions,
        )
    }
}
