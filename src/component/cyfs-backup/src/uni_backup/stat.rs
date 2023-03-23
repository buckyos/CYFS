use crate::meta::*;
use cyfs_base::*;
use cyfs_lib::*;

pub struct UniBackupStat {
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
}

impl UniBackupStat {
    pub fn new(noc: NamedObjectCacheRef, ndc: NamedDataCacheRef) -> Self {
        Self { noc, ndc }
    }

    pub async fn stat(&self) -> BuckyResult<ObjectArchiveDataMetas> {
        let stat = self.noc.stat().await?;
        let objects = ObjectArchiveDataMeta {
            count: stat.count,
            bytes: 0,
        };

        let stat = self.ndc.stat().await?;
        let chunks = ObjectArchiveDataMeta {
            count: stat.count,
            bytes: 0,
        };

        let stat = ObjectArchiveDataMetas { objects, chunks };
        Ok(stat)
    }
}
