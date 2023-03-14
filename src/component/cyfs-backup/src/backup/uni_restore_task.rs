use crate::data::*;
use crate::key_data::*;
use crate::meta::*;
use crate::restore::StackLocalObjectRestorer;
use crate::restore::*;
use crate::uni_backup::UniRestoreManager;
use cyfs_base::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct UniBackupParams {
    pub id: u64,
    pub cyfs_root: String,
    pub isolate: String,
    pub archive: PathBuf,
}

#[derive(Clone)]
pub struct UniRestoreTask {
    id: u64,
    isolate: String,
}

impl UniRestoreTask {
    pub async fn run(&self, params: UniBackupParams) -> BuckyResult<()> {
        info!("will uni restore: {:?}", params);
        
        let loader = ArchiveLocalFileLoader::load(params.archive).await?;

        let loader: BackupDataLoaderRef = Arc::new(Box::new(loader));

        let meta_str = loader.meta().await?;

        let meta: ObjectArchiveMetaForUniBackup = serde_json::from_str(&meta_str).map_err(|e| {
            let msg = format!("invalid uni meta info format! value={}, {}", meta_str, e,);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        let cyfs_root = PathBuf::from(&params.cyfs_root);
        let restorer = StackLocalObjectRestorer::create(cyfs_root, &params.isolate).await?;
        let restorer = Arc::new(Box::new(restorer) as Box<dyn ObjectRestorer>);

        if meta.key_data.len() > 0 {
            let key_data_restore =
                KeyDataRestoreManager::new(meta.key_data, loader.clone(), restorer.clone());
            key_data_restore.run().await?;
        }

        let uni_restore = UniRestoreManager::new(params.id, loader.clone(), restorer.clone());
        uni_restore.run().await?;

        Ok(())
    }
}
