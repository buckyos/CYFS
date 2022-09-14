use cyfs_base::{BuckyError, DeviceId, ObjectId};
use cyfs_lib::*;
use rusqlite::Row;
use std::str::FromStr;

pub(super) struct SqliteObjectCacheData {
    // 来源协议
    pub protocol: String,

    // 来源对象
    pub device_id: String,

    // 对象id
    pub object_id: String,

    // 对象所属dec，可以为空
    pub dec_id: String,

    // 对象内容
    pub object_raw: Vec<u8>,

    // put_flags/get_flags
    pub flags: u32,

    pub create_time: i64,
    pub update_time: i64,
    pub insert_time: i64,

    pub rank: u8,
}

use std::convert::{TryFrom, TryInto};

impl TryFrom<&Row<'_>> for SqliteObjectCacheData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            object_id: row.get(0)?,
            protocol: row.get(1)?,
            device_id: row.get(4)?,
            dec_id: row.get(5)?,

            create_time: row.get(8)?,
            update_time: row.get(9)?,
            insert_time: row.get(10)?,

            rank: row.get(12)?,

            flags: row.get(13)?,
            object_raw: row.get(14)?,
        };

        Ok(data)
    }
}

impl TryInto<ObjectCacheData> for SqliteObjectCacheData {
    type Error = BuckyError;

    fn try_into(self) -> Result<ObjectCacheData, Self::Error> {
        let dec_id = if self.dec_id.len() > 0 {
            Some(ObjectId::from_str(&self.dec_id)?)
        } else {
            None
        };

        // TODO 移除protocol字段
        let protocol = match RequestProtocol::from_str(self.protocol.as_str()) {
            Ok(v) => v,
            Err(_e) => RequestProtocol::Native,
        };

        let mut ret = ObjectCacheData {
            protocol,
            object_id: ObjectId::from_str(&self.object_id)?,
            source: DeviceId::from_str(&self.device_id)?,
            dec_id,
            create_time: self.create_time as u64,
            update_time: self.update_time as u64,
            insert_time: self.insert_time as u64,
            flags: self.flags,
            rank: self.rank,
            object_raw: Some(self.object_raw),
            object: None,
        };

        ret.rebuild_object()?;

        Ok(ret)
    }
}

pub(super) struct SqliteSyncObjectData {
    // 对象id
    pub object_id: String,

    pub update_time: i64,
    pub insert_time: i64,
}

impl TryFrom<&Row<'_>> for SqliteSyncObjectData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            object_id: row.get(0)?,

            insert_time: row.get(1)?,
            update_time: row.get(2)?,
        };

        Ok(data)
    }
}

impl TryInto<SyncObjectData> for SqliteSyncObjectData {
    type Error = BuckyError;

    fn try_into(self) -> Result<SyncObjectData, Self::Error> {
        let ret = SyncObjectData {
            object_id: ObjectId::from_str(&self.object_id)?,

            update_time: self.update_time as u64,
            seq: self.insert_time as u64,
        };

        Ok(ret)
    }
}
