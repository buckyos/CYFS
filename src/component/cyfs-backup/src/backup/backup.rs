use super::status::BackupStatus;
use super::uni_backup_task::*;
use cyfs_base::*;
use cyfs_bdt::ChunkReaderRef;
use cyfs_lib::*;

use std::sync::{Arc, Mutex};

pub struct BackupManager {
    isolate: String,
    state_default_isolate: ObjectId,
    noc: NamedObjectCacheRef,
    ndc: NamedDataCacheRef,
    loader: ObjectTraverserLoaderRef,

    // all backup tasks
    tasks: Mutex<Vec<UniBackupTask>>,
}

impl BackupManager {
    pub fn new(
        isolate: &str,
        state_default_isolate: ObjectId,
        noc: NamedObjectCacheRef,
        ndc: NamedDataCacheRef,
        chunk_reader: ChunkReaderRef,
    ) -> Self {
        let loader = ObjectTraverserLocalLoader::new(noc.clone(), chunk_reader).into_reader();
        Self {
            isolate: isolate.to_owned(),
            state_default_isolate,
            noc,
            ndc,
            loader,

            tasks: Mutex::new(vec![]),
        }
    }

    fn create_uni_backup_task(&self, params: &UniBackupParams) -> BuckyResult<UniBackupTask> {
        let task = UniBackupTask::new(
            params.id,
            &self.isolate,
            self.noc.clone(),
            self.ndc.clone(),
            self.loader.clone(),
        );

        {
            let mut tasks = self.tasks.lock().unwrap();
            if tasks.iter().find(|item| item.id() == params.id).is_some() {
                let msg = format!("backup task already exists! task={}", params.id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }

            tasks.push(task.clone());
        }

        Ok(task)
    }

    pub async fn run_uni_backup(&self, params: UniBackupParams) -> BuckyResult<()> {
        let task = self.create_uni_backup_task(&params)?;

        task.run(params).await
    }

    pub async fn start_uni_backup(&self, params: UniBackupParams) -> BuckyResult<()> {
        let task = self.create_uni_backup_task(&params)?;

        async_std::task::spawn(async move {
            let id = params.id;
            match task.run(params).await {
                Ok(()) => {
                    info!("run uni backup task complete! task={}", id);
                }
                Err(e) => {
                    error!("run uni backup task failed! task={}, {}", id, e);
                }
            }
        });

        Ok(())
    }

    pub fn get_task_status(&self, id: u64) -> BuckyResult<BackupStatus> {
        let status = {
            let tasks = self.tasks.lock().unwrap();
            let ret = tasks.iter().find(|item| item.id() == id);
            if ret.is_none() {
                let msg = format!("backup task not exists! task={}", id);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            ret.unwrap().status()
        };

        Ok(status)
    }
}

pub type BackupManagerRef = Arc<BackupManager>;
