use cyfs_base::*;
use cyfs_lib::*;

use rusqlite::Row;
use std::convert::{TryFrom, TryInto};

pub(super) struct SqlitePostionCacheData {

    // 对象id
    pub id: String,

    pub pos: String,
    pub pos_type: u8,
    pub direction: u8,

    pub insert_time: i64,
    pub update_time: i64,
    
    pub flags: u32,
}

impl TryFrom<&Row<'_>> for SqlitePostionCacheData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            id: row.get(0)?,
            pos: row.get(1)?,
            pos_type: row.get(2)?,
            direction: row.get(3)?,

            insert_time: row.get(4)?,
            update_time: row.get(5)?,
            
            flags: row.get(6)?,
        };

        Ok(data)
    }
}

impl TryInto<TrackerPositionCacheData> for SqlitePostionCacheData {
    type Error = BuckyError;

    fn try_into(self) -> Result<TrackerPositionCacheData, Self::Error> {

        let direction = TrackerDirection::from(self.direction);
        let pos = TrackerPostion::try_from((self.pos_type, self.pos))?;

        let ret = TrackerPositionCacheData {
            pos,
            direction,
            insert_time: self.insert_time as u64,
            flags: self.flags,
        };

        Ok(ret)
    }
}