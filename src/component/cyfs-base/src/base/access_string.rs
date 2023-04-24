use crate::*;
use intbits::Bits;
use std::convert::TryInto;
use std::fmt::{Formatter};
use std::str::FromStr;
use itertools::Itertools;
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::de::{Error, SeqAccess, Visitor};

const ACCESS_GROUP_MASK: u32 = 0b111 << 29;

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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessPermission {
    Call = 0b001,
    Write = 0b010,
    Read = 0b100,
}

impl AccessPermission {
    pub fn bit(&self) -> u8 {
        match *self {
            Self::Call => 0,
            Self::Write => 1,
            Self::Read => 2,
        }
    }

    pub fn test(&self, access: u8) -> bool {
        let b = *self as u8;
        access & b == b
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessPermissions {
    None = 0,
    CallOnly = 0b001,
    WriteOnly = 0b010,
    WriteAndCall = 0b011,
    ReadOnly = 0b100,
    ReadAndCall = 0b101,
    ReadAndWrite = 0b110,
    Full = 0b111,
}

impl AccessPermissions {
    pub fn as_str(&self) -> &'static str {
        match &self {
            Self::None => "---",
            Self::CallOnly => "--x",
            Self::WriteOnly => "-w-",
            Self::WriteAndCall => "-wx",
            Self::ReadOnly => "r--",
            Self::ReadAndCall => "r-x",
            Self::ReadAndWrite => "rw-",
            Self::Full => "rwx",
        }
    }

    pub fn format_u8(v: u8) -> std::borrow::Cow<'static, str> {
        match TryInto::<AccessPermissions>::try_into(v) {
            Ok(v) => std::borrow::Cow::Borrowed(v.as_str()),
            Err(_) => {
                let s = format!("{:o}", v);
                std::borrow::Cow::Owned(s)
            }
        }
    }

    pub fn test_op(&self, op_type: RequestOpType) -> bool {
        let access = Into::<AccessPermission>::into(op_type);
        access.test(*self as u8)
    }
}

impl TryFrom<u8> for AccessPermissions {
    type Error = BuckyError;
    fn try_from(v: u8) -> BuckyResult<Self> {
        if v > AccessPermissions::Full as u8 {
            let msg = format!("invalid AccessPermissions value: {}", v);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let ret: Self = unsafe { ::std::mem::transmute(v) };
        Ok(ret)
    }
}

impl FromStr for AccessPermissions {
    type Err = BuckyError;
    fn from_str(value: &str) -> BuckyResult<Self> {
        match value {
            "---" => Ok(AccessPermissions::None),
            "--x" => Ok(AccessPermissions::CallOnly),
            "-w-" => Ok(AccessPermissions::WriteOnly),
            "-wx" => Ok(AccessPermissions::WriteAndCall),
            "r--" => Ok(AccessPermissions::ReadOnly),
            "r-x" => Ok(AccessPermissions::ReadAndCall),
            "rw-" => Ok(AccessPermissions::ReadAndWrite),
            "rwx" => Ok(AccessPermissions::Full),
            v @ _ => {
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, format!("invalid access permissions {}", v)))
            }
        }
    }
}

impl std::fmt::Display for AccessPermissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for AccessPermissions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(self.as_str())
    }
}

struct AccessPermissionsVisitor;

impl<'de> Visitor<'de> for AccessPermissionsVisitor {
    type Value = AccessPermissions;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a string represent access permissions")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: Error {
        AccessPermissions::from_str(v).map_err(Error::custom)
    }
}

impl<'de> Deserialize<'de> for AccessPermissions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_string(AccessPermissionsVisitor)
    }
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessGroup {
    CurrentDevice = 0,
    CurrentZone = 3,
    FriendZone = 6,
    OthersZone = 9,

    OwnerDec = 12,
    OthersDec = 15,
}

pub const ACCESS_GROUP_LIST: &[AccessGroup; 6] = &[
    AccessGroup::CurrentDevice,
    AccessGroup::CurrentZone,
    AccessGroup::FriendZone,
    AccessGroup::OthersZone,
    AccessGroup::OwnerDec,
    AccessGroup::OthersDec,
];

impl AccessGroup {
    pub fn range(&self) -> std::ops::Range<u32> {
        let index = *self as u32;
        // println!("index={}, {:?}", index, self);
        index..index + 3
    }

    pub fn bit(&self, permission: AccessPermission) -> u32 {
        let index = *self as u32;
        index + permission.bit() as u32
    }
}

impl TryFrom<&str> for AccessGroup {
    type Error = BuckyError;

    fn try_from(value: &str) -> BuckyResult<Self> {
        match value {
            "CurrentDevice" => Ok(AccessGroup::CurrentDevice),
            "CurrentZone" => Ok(AccessGroup::CurrentZone),
            "FriendZone" => Ok(AccessGroup::FriendZone),
            "OthersZone" => Ok(AccessGroup::OthersZone),
            "OwnerDec" => Ok(AccessGroup::OwnerDec),
            "OthersDec" => Ok(AccessGroup::OthersDec),
            v @ _ => Err(BuckyError::new(BuckyErrorCode::InvalidParam, format!("invalid access group {}", v)))
        }
    }
}

pub struct AccessPair {
    group: AccessGroup,
    permissions: AccessPermissions,
}

#[derive(Clone, Eq, PartialEq)]
pub struct AccessString(u32);

impl std::fmt::Debug for AccessString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl AccessString {
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn value(&self) -> u32 {
        self.0
    }

    pub fn make(list: &[AccessPair]) -> Self {
        let mut ret = Self(0);
        list.iter()
            .for_each(|p| ret.set_group_permissions(p.group, p.permissions));
        ret
    }

    pub fn is_accessable(&self, group: AccessGroup, permission: AccessPermission) -> bool {
        self.0.bit(group.bit(permission))
    }

    pub fn set_group_permission(&mut self, group: AccessGroup, permission: AccessPermission) {
        self.0.set_bit(group.bit(permission), true);
    }

    pub fn clear_group_permission(&mut self, group: AccessGroup, permission: AccessPermission) {
        self.0.set_bit(group.bit(permission), false);
    }

    pub fn get_group_permissions(&self, group: AccessGroup) -> AccessPermissions {
        (self.0.bits(group.range()) as u8).try_into().unwrap()
    }

    pub fn set_group_permissions(&mut self, group: AccessGroup, permissions: AccessPermissions) {
        self.0.set_bits(group.range(), permissions as u32);
    }

    pub fn clear_group_permissions(&mut self, group: AccessGroup) {
        self.0.set_bits(group.range(), 0);
    }

    pub fn full_except_write() -> Self {
        static D: once_cell::sync::OnceCell<AccessString> = once_cell::sync::OnceCell::new();
        D.get_or_init(|| {
            Self::make(&[
                AccessPair {
                    group: AccessGroup::CurrentDevice,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::CurrentZone,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::FriendZone,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::OthersZone,
                    permissions: AccessPermissions::ReadAndCall,
                },
                AccessPair {
                    group: AccessGroup::OwnerDec,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::OthersDec,
                    permissions: AccessPermissions::ReadAndCall,
                },
            ])
        })
        .to_owned()
    }

    pub fn full() -> Self {
        static D: once_cell::sync::OnceCell<AccessString> = once_cell::sync::OnceCell::new();
        D.get_or_init(|| {
            Self::make(&[
                AccessPair {
                    group: AccessGroup::CurrentDevice,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::CurrentZone,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::FriendZone,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::OthersZone,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::OwnerDec,
                    permissions: AccessPermissions::Full,
                },
                AccessPair {
                    group: AccessGroup::OthersDec,
                    permissions: AccessPermissions::Full,
                },
            ])
        })
        .to_owned()
    }

    pub fn dec_default() -> Self {
        Self::make(&[
            AccessPair {
                group: AccessGroup::CurrentDevice,
                permissions: AccessPermissions::Full,
            },
            AccessPair {
                group: AccessGroup::CurrentZone,
                permissions: AccessPermissions::Full,
            },
            AccessPair {
                group: AccessGroup::FriendZone,
                permissions: AccessPermissions::Full,
            },
            AccessPair {
                group: AccessGroup::OwnerDec,
                permissions: AccessPermissions::Full,
            },
            AccessPair {
                group: AccessGroup::OthersDec,
                permissions: AccessPermissions::Full,
            },
        ])
    }

    fn to_string(&self) -> String {
        ACCESS_GROUP_LIST
            .iter()
            .map(|v| self.get_group_permissions(*v).as_str())
            .collect()
    }
}

impl TryFrom<&str> for AccessString {
    type Error = BuckyError;

    fn try_from(value: &str) -> BuckyResult<Self> {
        Self::from_str(value)
    }
}

impl FromStr for AccessString {
    type Err = BuckyError;

    fn from_str(value: &str) -> BuckyResult<Self> {
        let mut access = AccessString::new(0);
        for (mut chunk, group) in value.chars().filter(|c|c != &'_' && c != &' ').chunks(3).into_iter().zip(ACCESS_GROUP_LIST) {
            access.set_group_permissions(*group, AccessPermissions::from_str(chunk.join("").as_str())?);
        }

        Ok(access)
    }
}

impl Default for AccessString {
    fn default() -> Self {
        static D: once_cell::sync::OnceCell<AccessString> = once_cell::sync::OnceCell::new();
        D.get_or_init(|| Self::dec_default()).to_owned()
    }
}

impl std::fmt::Display for AccessString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Self::to_string(&self))
    }
}

impl Serialize for AccessString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

#[derive(Serialize, Deserialize)]
struct AccessGroupStruct {
    group: String,
    access: String
}

struct AccessStringVisitor;

impl<'de> Visitor<'de> for AccessStringVisitor {
    type Value = AccessString;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a string represent access string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> where E: Error {
        AccessString::try_from(v).map_err(Error::custom)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error> where A: SeqAccess<'de> {
        let mut ret = AccessString::default();
        while let Some(value) = seq.next_element::<AccessGroupStruct>()? {
            ret.set_group_permissions(AccessGroup::try_from(value.group.as_str()).map_err(Error::custom)?,
                                      AccessPermissions::from_str(value.access.as_str()).map_err(Error::custom)?);
        }

        Ok(ret)
    }
}

impl<'de> Deserialize<'de> for AccessString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_any(AccessStringVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_access_permissons() {
        let perm = AccessPermissions::ReadAndCall;
        assert!(perm.test_op(RequestOpType::Read));
        assert!(perm.test_op(RequestOpType::Call));
        assert!(!perm.test_op(RequestOpType::Write));

        let perm = AccessPermissions::None;
        assert!(!perm.test_op(RequestOpType::Read));
        assert!(!perm.test_op(RequestOpType::Call));
        assert!(!perm.test_op(RequestOpType::Write));

        let perm = AccessPermissions::Full;
        assert!(perm.test_op(RequestOpType::Read));
        assert!(perm.test_op(RequestOpType::Call));
        assert!(perm.test_op(RequestOpType::Write));
    }

    #[test]
    fn main() {

        let s = "rwxrwxrwx---rwxrwx";
        let v = AccessString::from_str(s).unwrap();
        assert_eq!(v.to_string(), s);
        
        let mut access_string = AccessString::default();
        println!("default={}", access_string);

        let ret = access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Read);
        assert!(ret);

        let ret = access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Call);
        assert!(ret);
        let ret = access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Read);
        assert!(ret);
        let ret = access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Write);
        assert!(ret);

        let ret = access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Call);
        assert!(ret);
        let ret = access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Read);
        assert!(ret);
        let ret = access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Write);
        assert!(ret);

        access_string.clear_group_permission(AccessGroup::OthersDec, AccessPermission::Read);
        let ret = access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Read);
        assert!(!ret);

        access_string.clear_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
        let ret = access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Call);
        assert!(!ret);

        let c = access_string.get_group_permissions(AccessGroup::CurrentZone);
        assert_eq!(c, AccessPermissions::Full);

        println!("{}", c);

        access_string.clear_group_permissions(AccessGroup::CurrentZone);
        let c = access_string.get_group_permissions(AccessGroup::CurrentZone);
        assert_eq!(c, AccessPermissions::None);

        access_string.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Call);
        access_string.set_group_permission(AccessGroup::CurrentZone, AccessPermission::Read);

        access_string.set_group_permissions(AccessGroup::CurrentZone, AccessPermissions::ReadAndCall);
        
        println!("{}", access_string);

        let access_string2 = AccessString::try_from(access_string.to_string().as_str());
        assert!(access_string2.is_ok());
        assert_eq!(access_string.value(), access_string2.unwrap().value());

        let c = access_string.get_group_permissions(AccessGroup::CurrentZone);
        assert_eq!(c, AccessPermissions::ReadAndCall);
        println!("{}", c);
    }
}
