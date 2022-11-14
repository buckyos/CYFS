use cyfs_base::*;
use cyfs_lib::*;

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
    // version 0
    pub create_dec_id: String,

    pub insert_time: i64,
    pub update_time: i64,

    pub object_update_time: Option<i64>,
    pub object_expired_time: Option<i64>,

    pub access_string: u32,

    // version 1
    pub object_type: u16,
    pub object_create_time: Option<u64>,

    pub dec_id: Option<Vec<u8>>,
    pub author: Option<Vec<u8>>,
    pub owner_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct NamedObjectMetaUpdateInfo {
    // version 0
    pub create_dec_id: ObjectId,

    pub insert_time: u64,
    pub update_time: u64,

    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,

    pub access_string: u32,

    // version 1
    pub object_type: u16,
    pub object_create_time: Option<u64>,

    pub owner_id: Option<ObjectId>,
    pub dec_id: Option<ObjectId>,
    pub author: Option<ObjectId>,
}

impl TryFrom<&Row<'_>> for NamedObjectMetaUpdateInfoRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        // trace!("will covert raw to NamedObjectMetaUpdateInfoRaw...");

        let data = Self {
            create_dec_id: row.get(0)?,

            insert_time: row.get(1)?,
            update_time: row.get(2)?,

            object_update_time: row.get(3)?,
            object_expired_time: row.get(4)?,

            access_string: row.get(5)?,

            // version 1
            object_type: row.get(6)?,
            object_create_time: row.get(7)?,
            owner_id: row.get(8)?,
            dec_id: row.get(9)?,
            author: row.get(10)?,
        };

        Ok(data)
    }
}

impl TryInto<NamedObjectMetaUpdateInfo> for NamedObjectMetaUpdateInfoRaw {
    type Error = BuckyError;
    fn try_into(self) -> Result<NamedObjectMetaUpdateInfo, Self::Error> {
        Ok(NamedObjectMetaUpdateInfo {
            // version 0
            create_dec_id: ObjectId::from_str(&self.create_dec_id)?,
            insert_time: self.insert_time as u64,
            update_time: self.update_time as u64,

            object_update_time: self.object_update_time.map(|v| v as u64),
            object_expired_time: self.object_expired_time.map(|v| v as u64),

            access_string: self.access_string,

            // version 1
            object_type: self.object_type,
            object_create_time: self.object_create_time,
            owner_id: convert_option_value(&self.owner_id)?,
            author: match self.author {
                Some(v) => Some(ObjectId::try_from(v)?),
                None => None,
            },
            dec_id: match self.dec_id {
                Some(v) => Some(ObjectId::try_from(v)?),
                None => None,
            },
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
            create_dec_id: row.get(0)?,

            access_string: row.get(1)?,
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
    pub object_type: u16,

    pub owner_id: Option<String>,
    pub create_dec_id: String,

    pub insert_time: u64,
    pub update_time: u64,

    pub object_create_time: Option<u64>,
    pub object_update_time: Option<u64>,
    pub object_expired_time: Option<u64>,

    // object related fields
    pub author: Option<Vec<u8>>,
    pub dec_id: Option<Vec<u8>>,

    pub storage_category: u8,
    pub context: Option<String>,

    pub last_access_rpath: Option<String>,
    pub access_string: u32,
}

impl TryFrom<&Row<'_>> for NamedObjectMetaDataRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            // version 0
            object_id: row.get(0)?,
            owner_id: row.get(1)?,
            create_dec_id: row.get(2)?,

            insert_time: row.get(3)?,
            update_time: row.get(4)?,

            object_update_time: row.get(5)?,
            object_expired_time: row.get(6)?,

            storage_category: row.get(7)?,
            context: row.get(8)?,

            last_access_rpath: row.get(10)?,
            access_string: row.get(11)?,

            // version 1
            object_type: row.get(12)?,
            object_create_time: row.get(13)?,
            author: row.get(14)?,
            dec_id: row.get(15)?,
        })
    }
}

fn convert_option_value<T: FromStr<Err = BuckyError>>(
    s: &Option<String>,
) -> BuckyResult<Option<T>> {
    match s {
        Some(s) => Ok(Some(T::from_str(s)?)),
        None => Ok(None),
    }
}

impl TryInto<NamedObjectMetaData> for NamedObjectMetaDataRaw {
    type Error = BuckyError;
    fn try_into(self) -> Result<NamedObjectMetaData, Self::Error> {
        Ok(NamedObjectMetaData {
            object_id: ObjectId::from_str(&self.object_id)?,
            object_type: self.object_type,
            owner_id: convert_option_value(&self.owner_id)?,
            create_dec_id: ObjectId::from_str(&self.create_dec_id)?,

            insert_time: self.insert_time,
            update_time: self.update_time,

            object_create_time: self.object_create_time,
            object_update_time: self.object_update_time,
            object_expired_time: self.object_expired_time,

            author: match self.author {
                Some(v) => Some(ObjectId::try_from(v)?),
                None => None,
            },
            dec_id: match self.dec_id {
                Some(v) => Some(ObjectId::try_from(v)?),
                None => None,
            },
            storage_category: NamedObjectStorageCategory::try_from(self.storage_category)
                .unwrap_or(NamedObjectStorageCategory::default()),

            context: self.context,

            last_access_rpath: self.last_access_rpath,
            access_string: self.access_string,
        })
    }
}

pub(crate) struct NamedObjectMetaUpdateInfoDataProvider<'a, 'b> {
    pub object_id: &'a ObjectId,
    pub info: &'b NamedObjectMetaUpdateInfo,
}

impl<'a, 'b> ObjectSelectorDataProvider for NamedObjectMetaUpdateInfoDataProvider<'a, 'b> {
    fn object_id(&self) -> &ObjectId {
        &self.object_id
    }
    fn obj_type(&self) -> u16 {
        self.info.object_type
    }

    fn object_dec_id(&self) -> &Option<ObjectId> {
        &self.info.dec_id
    }
    fn object_author(&self) -> &Option<ObjectId> {
        &self.info.author
    }
    fn object_owner(&self) -> &Option<ObjectId> {
        &self.info.owner_id
    }

    fn object_create_time(&self) -> Option<u64> {
        self.info.object_create_time
    }
    fn object_update_time(&self) -> Option<u64> {
        self.info.object_update_time
    }
    fn object_expired_time(&self) -> Option<u64> {
        self.info.object_expired_time
    }

    fn update_time(&self) -> &u64 {
        &self.info.update_time
    }
    fn insert_time(&self) -> &u64 {
        &self.info.insert_time
    }
}
