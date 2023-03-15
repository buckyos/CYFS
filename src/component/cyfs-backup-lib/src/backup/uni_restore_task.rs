use crate::data::*;
use crate::key_data::*;
use crate::meta::*;
use crate::restore::StackLocalObjectRestorer;
use crate::restore::*;
use crate::uni_backup::*;
use cyfs_base::*;
use super::restore_status::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct UniRestoreParams {
    pub id: String,
    pub cyfs_root: String,
    pub isolate: String,
    pub archive: PathBuf,
}

#[derive(Clone)]
pub struct UniRestoreTask {
    id: String,

    status_manager: RestoreStatusManager,
}

impl UniRestoreTask {
    pub fn new(id: String) -> Self {
        Self {
            id,
            status_manager: RestoreStatusManager::new(),
        }
    }

    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn status(&self) -> RestoreStatus {
        self.status_manager.status()
    }

    pub async fn run(&self, params: UniRestoreParams) -> BuckyResult<()> {
        info!("will uni restore: {:?}", params);
        let ret = self.run_restore(params).await;

        let r = match ret.as_ref() {
            Ok(_) => { Ok(()) },
            Err(e) => {
                Err(e.clone())
            }
        };

        self.status_manager.on_complete(ret);

        self.status_manager.update_phase(RestoreTaskPhase::Complete);

        r
    }

    async fn run_restore(&self, params: UniRestoreParams) -> BuckyResult<RestoreResult> {
        self.status_manager.update_phase(RestoreTaskPhase::LoadAndVerify);

        let loader = ArchiveLocalFileLoader::load(params.archive).await?;

        let loader: BackupDataLoaderRef = Arc::new(Box::new(loader));

        let meta_str = loader.meta().await?;

        let meta: ObjectArchiveMetaForUniBackup = serde_json::from_str(&meta_str).map_err(|e| {
            let msg = format!("invalid uni meta info format! value={}, {}", meta_str, e,);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        self.status_manager.init_stat(&meta);

        self.status_manager.update_phase(RestoreTaskPhase::RestoreKeyData);

        let cyfs_root = PathBuf::from(&params.cyfs_root);
        let restorer = StackLocalObjectRestorer::create(cyfs_root, &params.isolate).await?;
        let restorer = Arc::new(Box::new(restorer) as Box<dyn ObjectRestorer>);

        let filter = UniRestoreDataFilter::new();

        if meta.key_data.len() > 0 {
            filter.append_key_data_chunks(&meta.key_data);

            let key_data_restore =
                KeyDataRestoreManager::new(meta.key_data.clone(), loader.clone(), restorer.clone(), self.status_manager.clone());
            key_data_restore.run().await?;
        }

        let uni_restore = UniRestoreManager::new(params.id, 
            loader.clone(), restorer.clone(), filter, self.status_manager.clone());
        uni_restore.run().await?;

        let result = RestoreResult {
            index: loader.index().await,
            uni_meta: Some(meta),
        };

        Ok(result)
    }
}
