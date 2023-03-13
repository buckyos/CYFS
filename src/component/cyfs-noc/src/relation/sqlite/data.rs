use cyfs_base::*;
use cyfs_lib::*;

use rusqlite::Row;
use std::str::FromStr;

pub(super) struct NamedObjectRelationCacheDataRaw {
    // version 0
    pub target_object_id: String,
}

impl TryFrom<&Row<'_>> for NamedObjectRelationCacheDataRaw {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        // trace!("will covert raw to NamedObjectMetaUpdateInfoRaw...");

        let data = Self {
            target_object_id: row.get(0)?,
        };

        Ok(data)
    }
}

impl TryInto<NamedObjectRelationCacheData> for NamedObjectRelationCacheDataRaw {
    type Error = BuckyError;
    fn try_into(self) -> Result<NamedObjectRelationCacheData, Self::Error> {
        Ok(NamedObjectRelationCacheData {
            // version 0
            target_object_id: Some(ObjectId::from_str(&self.target_object_id)?),
        })
    }
}
