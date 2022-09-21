use crate::*;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::str::FromStr;

#[derive(Clone, Copy, Eq, Debug, PartialEq)]
pub enum RequestProtocol {
    Native,
    Meta,
    Sync,
    HttpBdt,
    HttpLocal,
    HttpLocalAuth,
    DatagramBdt,
    // bdt层的chunk数据传输
    DataBdt,
}

impl RequestProtocol {
    pub fn is_local(&self) -> bool {
        match *self {
            Self::Native | Self::HttpLocal | Self::HttpLocalAuth => true,
            Self::HttpBdt | Self::DatagramBdt | Self::DataBdt => false,
            Self::Meta | Self::Sync => false,
        }
    }

    pub fn is_remote(&self) -> bool {
        !self.is_local()
    }

    pub fn is_require_acl(&self) -> bool {
        match *self {
            Self::HttpBdt | Self::DatagramBdt | Self::DataBdt => true,
            Self::Native | Self::HttpLocal | Self::Meta | Self::Sync | Self::HttpLocalAuth => false,
        }
    }

    pub fn as_str(&self) -> &str {
        match *self {
            Self::Native => "native",
            Self::Meta => "meta",
            Self::Sync => "sync",
            Self::HttpBdt => "http-bdt",
            Self::HttpLocal => "http-local",
            Self::HttpLocalAuth => "http-local-auth",
            Self::DatagramBdt => "datagram-bdt",
            Self::DataBdt => "data-bdt",
        }
    }
}

impl ToString for RequestProtocol {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for RequestProtocol {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "native" => Self::Native,
            "meta" => Self::Meta,
            "sync" => Self::Sync,
            "http-bdt" => Self::HttpBdt,
            "http-local" => Self::HttpLocal,
            "http-local-auth" => Self::HttpLocalAuth,
            "datagram-bdt" => Self::DatagramBdt,
            "data-bdt" => Self::DataBdt,
            v @ _ => {
                let msg = format!("unknown non input protocol: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(ret)
    }
}

// source device's zone info
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DeviceZoneCategory {
    CurrentDevice = 0,
    CurrentZone = 1,
    FriendZone = 2,
    OtherZone = 3,
}

impl DeviceZoneCategory {
    pub fn as_str(&self) -> &str {
        match self {
            Self::CurrentDevice => "current-device",
            Self::CurrentZone => "current-zone",
            Self::FriendZone => "friend-zone",
            Self::OtherZone => "other-zone",
        }
    }

    pub fn is_included(&self, target: Self) -> bool {
        *self as u8 <= target as u8
    }
}

impl ToString for DeviceZoneCategory {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for DeviceZoneCategory {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "current-device" => Self::CurrentDevice,
            "current-zone" => Self::CurrentZone,
            "friend-zone" => Self::FriendZone,
            "other-zone" => Self::OtherZone,
            _ => {
                let msg = format!("unknown device zone category: {}", s);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(ret)
    }
}
impl Into<AccessGroup> for DeviceZoneCategory {
    fn into(self) -> AccessGroup {
        match self {
            DeviceZoneCategory::CurrentDevice => AccessGroup::CurrentDevice,
            DeviceZoneCategory::CurrentZone => AccessGroup::CurrentZone,
            DeviceZoneCategory::FriendZone => AccessGroup::FriendZone,
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

impl DeviceZoneInfo {
    pub fn new_local() -> Self {
        Self {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentDevice,
        }
    }

    pub fn new_current_zone() -> Self {
        Self {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::CurrentZone,
        }
    }

    pub fn new_friend_zone() -> Self {
        Self {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::FriendZone,
        }
    }

    pub fn new_other_zone() -> Self {
        Self {
            device: None,
            zone: None,
            zone_category: DeviceZoneCategory::OtherZone,
        }
    }

    pub fn is_current_device(&self) -> bool {
        match self.zone_category {
            DeviceZoneCategory::CurrentDevice => true,
            _ => false,
        }
    }

    pub fn is_current_zone(&self) -> bool {
        match self.zone_category {
            DeviceZoneCategory::CurrentDevice | DeviceZoneCategory::CurrentZone => true,
            _ => false,
        }
    }

    pub fn is_friend_zone(&self) -> bool {
        match self.zone_category {
            DeviceZoneCategory::CurrentDevice
            | DeviceZoneCategory::CurrentZone
            | DeviceZoneCategory::FriendZone => true,
            _ => false,
        }
    }
}

// The identy info of a request
#[derive(Clone)]
pub struct RequestSourceInfo {
    pub protocol: RequestProtocol,
    pub zone: DeviceZoneInfo,
    pub dec: ObjectId,

    // is passed the acl verified
    pub verified: bool,
}

impl std::fmt::Debug for RequestSourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self, f)
    }
}

impl std::fmt::Display for RequestSourceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "protocol={}, zone=({:?}-{:?}-{:?}), dec={}, verified={}",
            self.protocol.as_str(),
            self.zone.zone_category,
            self.zone.device,
            self.zone.zone,
            self.dec,
            self.verified,
        )
    }
}

impl RequestSourceInfo {
    pub fn new_local_system() -> Self {
        Self {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo::new_local(),
            dec: get_system_dec_app().to_owned(),
            verified: false,
        }
    }

    pub fn new_local_dec(dec: Option<ObjectId>) -> Self {
        Self {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo::new_local(),
            dec: dec.unwrap_or(get_system_dec_app().to_owned()),
            verified: false,
        }
    }

    pub fn new_zone_dec(dec: Option<ObjectId>) -> Self {
        Self {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo::new_current_zone(),
            dec: dec.unwrap_or(get_system_dec_app().to_owned()),
            verified: false,
        }
    }

    pub fn new_friend_zone_dec(dec: Option<ObjectId>) -> Self {
        Self {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo::new_friend_zone(),
            dec: dec.unwrap_or(get_system_dec_app().to_owned()),
            verified: false,
        }
    }

    pub fn new_other_zone_dec(dec: Option<ObjectId>) -> Self {
        Self {
            protocol: RequestProtocol::Native,
            zone: DeviceZoneInfo::new_other_zone(),
            dec: dec.unwrap_or(get_system_dec_app().to_owned()),
            verified: false,
        }
    }

    pub fn protocol(mut self, protocol: RequestProtocol) -> Self {
        self.protocol = protocol;
        self
    }

    pub fn set_dec(&mut self, dec_id: Option<ObjectId>) {
        self.dec = dec_id.unwrap_or(get_system_dec_app().to_owned());
    }

    pub fn dec(mut self, dec_id: Option<ObjectId>) -> Self {
        self.set_dec(dec_id);
        self
    }

    pub fn is_system_dec(&self) -> bool {
        self.dec == *get_system_dec_app()
    }

    // return none if is system dec
    pub fn get_opt_dec(&self) -> Option<&ObjectId> {
        if self.is_system_dec() {
            None
        } else {
            Some(&self.dec)
        }
    }

    pub fn set_verified(&mut self) {
        assert!(!self.verified);
        self.verified = true;
    }

    pub fn is_verified(&self) -> bool {
        self.verified
    }

    pub fn check_target_dec_permission(&self, op_target_dec: &Option<ObjectId>) -> bool {
        if self.is_system_dec() {
            true
        } else {
            match op_target_dec {
                Some(target) => self.compare_dec(target),
                None => {
                    // target_dec_id is none then equal as current dec
                    true
                }
            }
        }
    }

    pub fn is_current_device(&self) -> bool {
        self.zone.is_current_device()
    }

    pub fn is_current_zone(&self) -> bool {
        self.zone.is_current_zone()
    }

    pub fn compare_zone_category(&self, zone_category: DeviceZoneCategory) -> bool {
        self.zone.zone_category.is_included(zone_category)
    }

    pub fn compare_zone(&self, zone: &ObjectId) -> bool {
        self.zone.device.as_ref().map(|v| v.object_id()) == Some(zone)
            || self.zone.zone.as_ref() == Some(zone)
    }

    pub fn compare_dec(&self, dec: &ObjectId) -> bool {
        self.dec == *dec
    }

    pub fn mask(&self, dec_id: &ObjectId, permissions: impl Into<AccessPermissions>) -> u32 {
        let permissions = permissions.into();
        let mut access = AccessString::new(0);
        if self.dec == *dec_id {
            access.set_group_permissions(AccessGroup::OwnerDec, permissions);
        } else {
            access.set_group_permissions(AccessGroup::OthersDec, permissions);
        }

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }

    pub fn owner_dec_mask(&self, permissions: impl Into<AccessPermissions>) -> u32 {
        let permissions = permissions.into();

        let mut access = AccessString::new(0);
        access.set_group_permissions(AccessGroup::OwnerDec, permissions);

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }

    pub fn other_dec_mask(&self, permissions: impl Into<AccessPermissions>) -> u32 {
        let permissions = permissions.into();

        let mut access = AccessString::new(0);
        access.set_group_permissions(AccessGroup::OthersDec, permissions);

        let group = self.zone.zone_category.into();
        access.set_group_permissions(group, permissions);

        access.value()
    }

    pub fn check_current_zone(&self, service: &str) -> BuckyResult<()> {
        if !self.is_current_zone() {
            let msg = format!(
                "{} service valid only in current zone! source device={:?}, category={}",
                service,
                self.zone.device,
                self.zone.zone_category.as_str(),
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }

    pub fn check_current_device(&self, service: &str) -> BuckyResult<()> {
        if !self.is_current_device() {
            let msg = format!(
                "{} service valid only on current device! source device={:?}, category={}",
                service,
                self.zone.device,
                self.zone.zone_category.as_str(),
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        Ok(())
    }
}

impl JsonCodec<Self> for DeviceZoneInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_option_string_field(&mut obj, "device", self.device.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "zone", self.zone.as_ref());
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "zone_category",
            self.zone_category.as_str(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            device: JsonCodecHelper::decode_option_string_field(obj, "device")?,
            zone: JsonCodecHelper::decode_option_string_field(obj, "zone")?,
            zone_category: JsonCodecHelper::decode_string_field(obj, "zone_category")?,
        })
    }
}

impl JsonCodec<Self> for RequestSourceInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "zone", &self.zone);
        JsonCodecHelper::encode_string_field(&mut obj, "dec", &self.dec);
        JsonCodecHelper::encode_string_field(&mut obj, "protocol", &self.protocol);
        JsonCodecHelper::encode_bool_field(&mut obj, "verified", self.verified);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            zone: JsonCodecHelper::decode_field(obj, "zone")?,
            dec: JsonCodecHelper::decode_string_field(obj, "dec")?,
            protocol: JsonCodecHelper::decode_string_field(obj, "protocol")?,
            verified: JsonCodecHelper::decode_bool_field(obj, "verified")?,
        })
    }
}

impl Serialize for DeviceZoneCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for DeviceZoneCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn other_dec_read() {
        let dec = ObjectId::default();
        let source = RequestSourceInfo {
            zone: DeviceZoneInfo {
                device: None,
                zone: None,
                zone_category: DeviceZoneCategory::CurrentDevice,
            },
            dec,
            protocol: RequestProtocol::Native,
            verified: false,
        };

        let system = ObjectId::default();
        let mask = source.mask(&system, RequestOpType::Read);

        let default = AccessString::default().value();
        assert_ne!(default & mask, mask)
    }
}
