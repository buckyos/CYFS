use super::path::GlobalStatePathHelper;
use cyfs_base::*;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalStatePathStorageState {
    Concrete = 0,
    Virtual = 1,
}

impl Into<u8> for GlobalStatePathStorageState {
    fn into(self) -> u8 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

impl std::convert::TryFrom<u8> for GlobalStatePathStorageState {
    type Error = BuckyError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let ret = match value {
            0 => Self::Concrete,
            1 => Self::Virtual,
            _ => {
                let msg = format!("unknown GlobalStatePathStorageState value: {}", value);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

impl Serialize for GlobalStatePathStorageState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.clone().into())
    }
}

impl<'de> Deserialize<'de> for GlobalStatePathStorageState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u8(TU8Visitor::<Self>::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct GlobalStatePathConfigItem {
    pub path: String,

    // 要求存储状态，如为virtual则重建时会跳过。
    pub storage_state: Option<GlobalStatePathStorageState>,

    // 重建深度.0表示无引用深度，1表示会重建其引用的1层对象。不配置则根据对象的Selector确定初始重建深度。对大文件不自动重建，需要手动将depth设置为1.
    pub depth: Option<u8>,
}

impl GlobalStatePathConfigItem {
    pub fn try_fix_path(&mut self) {
        self.path = GlobalStatePathHelper::fix_path(&self.path).to_string();
    }
}

pub struct GlobalStatePathConfigItemValue {
    pub storage_state: Option<GlobalStatePathStorageState>,
    pub depth: Option<u8>,
}