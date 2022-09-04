use super::access::*;
use cyfs_base::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestOpType {
    Read,
    Write,
    Call,
}

impl Into<AccessPermissions> for RequestOpType {
    fn into(self) -> AccessPermissions {
        match self {
            Self::Read => AccessPermissions::ReadOnly,
            Self::Write => AccessPermissions::WriteOnly,
            Self::Call => AccessPermissions::CallOnly,
        }
    }
}

impl Into<AccessPermission> for RequestOpType {
    fn into(self) -> AccessPermission {
        match self {
            Self::Read => AccessPermission::Read,
            Self::Write => AccessPermission::Write,
            Self::Call => AccessPermission::Call,
        }
    }
}

// source device's zone info
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceZoneCategory {
    CurrentDevice,
    CurrentZone,
    FriendsZone,
    OtherZone,
}

impl Into<AccessGroup> for DeviceZoneCategory {
    fn into(self) -> AccessGroup {
        match self {
            DeviceZoneCategory::CurrentDevice => AccessGroup::CurrentDevice,
            DeviceZoneCategory::CurrentZone => AccessGroup::CurentZone,
            DeviceZoneCategory::FriendsZone => AccessGroup::FriendZone,
            DeviceZoneCategory::OtherZone => AccessGroup::OthersZone,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeviceZoneInfo {
    pub device: Option<DeviceId>,
    pub zone: Option<ObjectId>,
    pub zone_category: DeviceZoneCategory,
}

// The identy info of a request
#[derive(Clone, Debug)]
pub struct RequestSourceInfo {
    pub zone: DeviceZoneInfo,
    pub dec: ObjectId,
}

impl std::fmt::Display for RequestSourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "zone=({:?}-{:?}-{:?}),dec={}",
            self.zone.zone_category, self.zone.device, self.zone.zone, self.dec
        )
    }
}

impl RequestSourceInfo {
    pub fn compare_zone(&self, zone: &ObjectId) -> bool {
        self.zone.device.as_ref().map(|v| v.object_id()) == Some(zone)
            || self.zone.zone.as_ref() == Some(zone)
    }

    pub fn mask(&self, dec_id: &ObjectId, op_type: RequestOpType) -> u32 {
        let permissions = op_type.into();

        let mut access = AccessString::new(0);
        if *dec_id == self.dec {
            access.set_group_permissions(AccessGroup::OwnerDec, permissions);
        } else {
            access.set_group_permissions(AccessGroup::OthersDec, permissions);
        }

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }

    pub fn owner_dec_mask(&self, op_type: RequestOpType) -> u32 {
        let permissions = op_type.into();

        let mut access = AccessString::new(0);
        access.set_group_permissions(AccessGroup::OwnerDec, permissions);

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }

    pub fn other_dec_mask(&self, op_type: RequestOpType) -> u32 {
        let permissions = op_type.into();

        let mut access = AccessString::new(0);
        access.set_group_permissions(AccessGroup::OthersDec, permissions);

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }
}
