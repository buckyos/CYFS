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

// source device's zone info
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceZoneCategory {
    CurrentDevice,
    CurrentZone,
    FriendsZone,
    OtherZone,
}

#[derive(Clone, Debug)]
pub struct DeviceZoneInfo {
    pub device_id: DeviceId,
    pub zone_category: DeviceZoneCategory,
}

// The identy info of a request
#[derive(Clone, Debug)]
pub struct RequestSourceInfo {
    device: DeviceZoneInfo,
    dec: ObjectId,
}

impl std::fmt::Display for RequestSourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "device=({:?}-{}),dec={}",
            self.device.zone_category, self.device.device_id, self.dec
        )
    }
}

impl RequestSourceInfo {
    pub fn mask(&self, dec_id: &ObjectId, op_type: RequestOpType) -> u32 {
        let permissions = op_type.into();

        let mut access = AccessString::new(0);
        if *dec_id == self.dec {
            access.set_group_permissions(AccessGroup::OwnerDec, permissions);
        } else {
            access.set_group_permissions(AccessGroup::OthersDec, permissions);
        }

        let group = match self.device.zone_category {
            DeviceZoneCategory::CurrentDevice => AccessGroup::CurrentDevice,
            DeviceZoneCategory::CurrentZone => AccessGroup::CurentZone,
            DeviceZoneCategory::FriendsZone => AccessGroup::FriendZone,
            DeviceZoneCategory::OtherZone => AccessGroup::OthersZone,
        };

        access.set_group_permissions(group, permissions);

        access.value()
    }
}
