
use cyfs_base::*;
use intbits::Bits;

const ACCESS_GROUP_MASK: u32 = 0b111 << 29;

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
    WirteAndCall = 0b011,
    ReadOnly = 0b100,
    ReadAndCall = 0b101,
    ReadAndWrite = 0b110,
    Full = 0b111,
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

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessGroup {
    CurrentDevice = 0,
    CurentZone = 3,
    FriendZone = 6,
    OthersZone = 9,

    OwnerDec = 12,
    OthersDec = 15,
}

impl AccessGroup {
    pub fn range(&self) -> std::ops::Range<u32> {
        let index = *self as u32;
        // println!("index={}, {:?}", index, self);
        index .. index + 3
    }

    pub fn bit(&self, permission: AccessPermission) -> u32 {
        let index = *self as u32;
        index + permission.bit() as u32
    }
}

pub struct AccessPair {
    group: AccessGroup,
    permissions: AccessPermissions, 
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessString(u32);

impl AccessString {
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn value(&self) -> u32 {
        self.0
    }
    
    pub fn make(list: &[AccessPair]) -> Self {
        let mut ret = Self(0);
        list.iter().for_each(|p| ret.set_group_permissions(p.group, p.permissions));
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
}


impl Default for AccessString {
    fn default() -> Self {
        Self::make(&[AccessPair {
            group: AccessGroup::CurrentDevice,
            permissions: AccessPermissions::Full,
        }, AccessPair {
            group: AccessGroup::CurentZone,
            permissions: AccessPermissions::Full,
        }, AccessPair {
            group: AccessGroup::FriendZone,
            permissions: AccessPermissions::ReadAndCall,
        }, AccessPair {
            group: AccessGroup::OwnerDec,
            permissions: AccessPermissions::ReadAndCall,
        }])
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use cyfs_base::*;
    
    
    #[test]
    fn main() {
        let mut access_string = AccessString::default();

        let ret= access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Call);
        assert!(ret);
        let ret= access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Read);
        assert!(ret);
        let ret= access_string.is_accessable(AccessGroup::CurrentDevice, AccessPermission::Write);
        assert!(ret);

        let ret= access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Call);
        assert!(!ret);
        let ret= access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Read);
        assert!(!ret);
        let ret= access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Write);
        assert!(!ret);

        access_string.set_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
        let ret= access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Call);
        assert!(ret);

        access_string.clear_group_permission(AccessGroup::OthersDec, AccessPermission::Call);
        let ret= access_string.is_accessable(AccessGroup::OthersDec, AccessPermission::Call);
        assert!(!ret);


        let c = access_string.get_group_permissions(AccessGroup::CurentZone);
        assert_eq!(c, AccessPermissions::Full);

        access_string.clear_group_permissions(AccessGroup::CurentZone);
        let c = access_string.get_group_permissions(AccessGroup::CurentZone);
        assert_eq!(c, AccessPermissions::None);

        access_string.set_group_permission(AccessGroup::CurentZone, AccessPermission::Call);
        access_string.set_group_permission(AccessGroup::CurentZone, AccessPermission::Read);
        let c = access_string.get_group_permissions(AccessGroup::CurentZone);
        assert_eq!(c, AccessPermissions::ReadAndCall);

    }
}