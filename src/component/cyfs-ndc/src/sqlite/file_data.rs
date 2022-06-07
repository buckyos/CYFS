use cyfs_base::*;
use cyfs_lib::*;
use rusqlite::Row;

use std::convert::{TryFrom, TryInto};
use std::str::FromStr;

pub(super) struct SqliteFileCacheData {
    pub hash: String,

    pub file_id: String,

    // 对象id
    pub length: i64,

    // 对象owner，可以为空
    pub owner: String,

    pub insert_time: i64,
    pub update_time: i64,

    pub flags: u32,
}

impl TryFrom<&Row<'_>> for SqliteFileCacheData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            hash: row.get(0)?,
            file_id: row.get(1)?,
            length: row.get(2)?,
            owner: row.get(3)?,

            insert_time: row.get(4)?,
            update_time: row.get(5)?,
            flags: row.get(6)?,
        };

        Ok(data)
    }
}

impl TryInto<FileCacheData> for SqliteFileCacheData {
    type Error = BuckyError;

    fn try_into(self) -> Result<FileCacheData, Self::Error> {
        let owner = if self.owner.len() > 0 {
            Some(ObjectId::from_str(&self.owner)?)
        } else {
            None
        };

        let file_id = FileId::from_str(&self.file_id).map_err(|e| {
            error!("convert to file id error: {}, {}", self.file_id, e);
            e
        })?;

        let ret = FileCacheData {
            hash: self.hash,
            file_id,
            length: self.length as u64,
            flags: self.flags,
            owner,

            quick_hash: None,
            dirs: None,
        };

        Ok(ret)
    }
}

pub(super) struct SqliteChunkCacheData {

    pub insert_time: i64,
    pub update_time: i64,
    pub last_access_time: i64,

    pub state: u8,

    pub flags: u32,
}

impl TryFrom<&Row<'_>> for SqliteChunkCacheData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            insert_time: row.get(0)?,
            update_time: row.get(1)?,
            last_access_time: row.get(2)?,

            state: row.get(3)?,
            flags: row.get(4)?,
        };

        Ok(data)
    }
}

impl SqliteChunkCacheData {
    pub fn into_chunk_data(self, chunk_id: ChunkId) -> BuckyResult<ChunkCacheData> {
        
        let state = ChunkState::try_from(self.state)?;
        let ret= ChunkCacheData {
            chunk_id,

            insert_time: self.insert_time as u64,
            update_time: self.update_time as u64,
            last_access_time: self.last_access_time as u64,

            state,
            flags: self.flags,

            ref_objects: None,
            trans_sessions: None,
        };

        Ok(ret)
    }
}