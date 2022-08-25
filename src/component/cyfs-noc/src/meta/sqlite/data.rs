use crate::prelude::NamedObjectStorageCategory;
use cyfs_base::*;

use super::super::meta::NamedObjectMetaData;

use rusqlite::{types::FromSql, Row};
use std::str::FromStr;

fn column_to_sql_value<T: FromSql>(row: &Row<'_>, index: usize) -> BuckyResult<T> {
    row.get(index).map_err(|e| {
        let msg = format!("noc meta query_row error: {}", e);
        error!("{}", msg);

        BuckyError::new(BuckyErrorCode::SqliteError, msg)
    })
}

fn column_to_option_sql_value<T: FromSql>(row: &Row<'_>, index: usize) -> BuckyResult<Option<T>> {
    row.get(index).map_err(|e| {
        let msg = format!("noc meta query_row error: {}", e);
        error!("{}", msg);

        BuckyError::new(BuckyErrorCode::SqliteError, msg)
    })
}

fn column_to_value<T: FromStr<Err = BuckyError>>(row: &Row<'_>, index: usize) -> BuckyResult<T> {
    let s: String = column_to_sql_value(row, index)?;

    T::from_str(&s)
}

fn column_to_option_value<T: FromStr<Err = BuckyError>>(
    row: &Row<'_>,
    index: usize,
) -> BuckyResult<Option<T>> {
    let s: Option<String> = column_to_sql_value(row, index)?;

    match s {
        Some(s) => Ok(Some(T::from_str(&s)?)),
        None => Ok(None),
    }
}

pub(super) struct NamedObjectMetaUpdateInfoRaw {
    pub create_dec_id: String,

    pub insert_time: u64,
    pub update_time: u64,

    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,

    pub access_string: u32,
}

pub(super) struct NamedObjectMetaUpdateInfo {
    pub create_dec_id: ObjectId,

    pub insert_time: u64,
    pub update_time: u64,

    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,

    pub access_string: u32,
}

impl TryFrom<&Row<'_>> for NamedObjectMetaUpdateInfoRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            create_dec_id: row.get(1)?,

            insert_time: row.get(2)?,
            update_time: row.get(3)?,

            object_update_time: row.get(4)?,
            object_expired_time: row.get(5)?,

            access_string: row.get(6)?,
        };

        Ok(data)
    }
}

impl TryInto<NamedObjectMetaUpdateInfo> for NamedObjectMetaUpdateInfoRaw {
    type Error = BuckyError;
    fn try_into(self) -> Result<NamedObjectMetaUpdateInfo, Self::Error> {
        Ok(NamedObjectMetaUpdateInfo {
            create_dec_id: ObjectId::from_str(&self.create_dec_id)?,
            insert_time: self.insert_time,
            update_time: self.update_time,

            object_update_time: self.object_update_time,
            object_expired_time: self.object_expired_time,

            access_string: self.access_string,
        })
    }
}


pub(super) struct NamedObjectMetaAccessInfoRaw {
    pub create_dec_id: String,

    pub access_string: u32,
}

pub(super) struct NamedObjectMetaAccessInfo {
    pub create_dec_id: ObjectId,

    pub access_string: u32,
}

impl TryFrom<&Row<'_>> for NamedObjectMetaAccessInfoRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = Self {
            create_dec_id: row.get(1)?,

            access_string: row.get(2)?,
        };

        Ok(data)
    }
}

impl TryInto<NamedObjectMetaAccessInfo> for NamedObjectMetaAccessInfoRaw {
    type Error = BuckyError;

    fn try_into(self) -> Result<NamedObjectMetaAccessInfo, Self::Error> {
        Ok(NamedObjectMetaAccessInfo {
            create_dec_id: ObjectId::from_str(&self.create_dec_id)?,

            access_string: self.access_string,
        })
    }
}

pub(super) struct NamedObjectMetaDataRaw {
    pub object_id: String,

    pub owner_id: Option<String>,
    pub create_dec_id: String,

    pub update_time: Option<u64>,
    pub expired_time: Option<u64>,

    pub storage_category: u8,
    pub context: Option<String>,

    pub last_access_rpath: Option<String>,
    pub access_string: u32,
}

impl TryFrom<&Row<'_>> for NamedObjectMetaDataRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            object_id: row.get(1)?,
            owner_id: row.get(2)?,
            create_dec_id: row.get(3)?,

            update_time: row.get(5)?,
            expired_time: row.get(6)?,

            storage_category: row.get(7)?,
            context: row.get(8)?,

            last_access_rpath: row.get(9)?,
            access_string: row.get(11)?,
        })
    }
}

fn convert_option_value<T: FromStr<Err = BuckyError>>(s: &Option<String>) -> BuckyResult<Option<T>> {
    match s {
        Some(s) => {
            Ok(Some(T::from_str(s)?))
        }
        None => Ok(None)
    }
}

impl TryInto<NamedObjectMetaData> for NamedObjectMetaDataRaw {
    type Error = BuckyError;
    fn try_into(self) -> Result<NamedObjectMetaData, Self::Error> {

        Ok(NamedObjectMetaData {
            object_id: ObjectId::from_str(&self.object_id)?,
            owner_id: convert_option_value(&self.owner_id)?,
            create_dec_id: ObjectId::from_str(&self.create_dec_id)?,

            update_time: self.update_time,
            expired_time: self.expired_time,

            storage_category: NamedObjectStorageCategory::try_from(self.storage_category).unwrap_or(NamedObjectStorageCategory::default()),

            context: self.context,

            last_access_rpath: self.last_access_rpath,
            access_string: self.access_string,
        })
    }
}