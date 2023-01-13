use super::task::*;
use crate::ndn::TaskGroupHelper;
use crate::trans_api::{DownloadTaskTracker, TransStore};
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::TransTaskInfo;
use cyfs_task_manager::*;

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::trans_proto::DownloadTaskState)]
pub struct DownloadTaskState {
    pub task_status: TaskStatus,
    pub err_code: Option<BuckyErrorCode>,
    pub speed: u64,
    pub upload_speed: u64,
    pub downloaded_progress: u64,
    pub sum_size: u64,
    pub group: Option<String>,
}

impl ProtobufTransform<super::trans_proto::DownloadTaskState> for DownloadTaskState {
    fn transform(
        value: crate::trans_api::local::trans_proto::DownloadTaskState,
    ) -> BuckyResult<Self> {
        Ok(Self {
            task_status: TaskStatus::try_from(value.task_status)?,
            err_code: value.err_code.map(|v| BuckyErrorCode::from(v)),
            speed: value.speed,
            upload_speed: value.upload_speed,
            downloaded_progress: value.download_progress,
            sum_size: value.sum_size,
            group: value.group,
        })
    }
}

impl ProtobufTransform<&DownloadTaskState> for super::trans_proto::DownloadTaskState {
    fn transform(value: &DownloadTaskState) -> BuckyResult<Self> {
        Ok(Self {
            task_status: value.task_status.into(),
            err_code: value.err_code.map(|v| v.into()),
            speed: value.speed,
            upload_speed: value.upload_speed,
            download_progress: value.downloaded_progress,
            sum_size: value.sum_size,
            group: value.group.clone(),
        })
    }
}

#[derive(Clone)]
pub(crate) struct DownloadTaskManager {
    stack: StackGuard,
    task_manager: Arc<TaskManager>,
    trans_store: Arc<TransStore>,
}

impl DownloadTaskManager {
    pub fn new(
        stack: StackGuard,
        named_data_components: &NamedDataComponents,
        task_manager: Arc<TaskManager>,
        trans_store: Arc<TransStore>,
    ) -> Self {
        task_manager
            .register_task_factory(DownloadChunkTaskFactory::new(
                stack.clone(),
                named_data_components.clone(),
                trans_store.clone(),
            ))
            .unwrap();
        task_manager
            .register_task_factory(DownloadFileTaskFactory::new(
                stack.clone(),
                named_data_components.clone(),
                trans_store.clone(),
            ))
            .unwrap();

        Self {
            stack,
            task_manager,
            trans_store,
        }
    }

    /*
    pub fn gen_task_id(obj_id: &ObjectId, local_path: Option<String>) -> TaskId {
        let mut sha256 = sha2::Sha256::new();
        sha256.input(DOWNLOAD_FILE_TASK.0.to_le_bytes());
        sha256.input(obj_id.as_slice());
        if local_path.is_some() {
            sha256.input(local_path.as_ref().unwrap().as_bytes());
        }
        sha256.result().into()
    }
    */

    pub async fn create_file_task(
        &self,
        source: DeviceId,
        dec_id: ObjectId,
        group: Option<String>,
        context: Option<String>,
        file: File,
        local_path: Option<String>,
        device_list: Vec<DeviceId>,
        referer: String,
    ) -> BuckyResult<TaskId> {
        let file_id = file.desc().calculate_id();
        if local_path.is_some() {
            log::info!(
                "create file task dec_id {} file {} local_path {}",
                dec_id.to_string(),
                file_id.to_string(),
                local_path.as_ref().unwrap()
            );
        } else {
            log::info!(
                "create file task dec_id {} file {}",
                dec_id.to_string(),
                file_id.to_string()
            );
        }

        let group = TaskGroupHelper::new_opt_with_dec(&dec_id, group.as_deref());
        let params = DownloadFileParam {
            dec_id: dec_id.clone(),
            file,
            device_list,
            referer,
            save_path: local_path.clone(),
            group,
            context,
        };

        let task_id = self
            .task_manager
            .create_task(dec_id.clone(), source.clone(), DOWNLOAD_FILE_TASK, params)
            .await?;
        // assert_eq!(task_id, Self::gen_task_id(&file_id, local_path));

        let mut conn = self.trans_store.create_connection().await?;
        conn.add_task_info(&task_id, &None, TaskStatus::Stopped, vec![(source, dec_id)])
            .await?;
        Ok(task_id)
    }

    pub async fn create_chunk_task(
        &self,
        source: DeviceId,
        dec_id: ObjectId,
        group: Option<String>,
        context: Option<String>,
        chunk_id: ChunkId,
        local_path: Option<String>,
        device_list: Vec<DeviceId>,
        referer: String,
    ) -> BuckyResult<TaskId> {
        if local_path.is_some() {
            log::info!(
                "create chunk task dec_id {} chunk {} local_path {}",
                dec_id.to_string(),
                chunk_id.to_string(),
                local_path.as_ref().unwrap()
            );
        } else {
            log::info!(
                "create chunk task dec_id {} chunk {}",
                dec_id.to_string(),
                chunk_id.to_string()
            );
        }

        let group = TaskGroupHelper::new_opt_with_dec(&dec_id, group.as_deref());
        let params = DownloadChunkParam {
            dec_id: dec_id.clone(),
            chunk_id,
            device_list,
            referer,
            save_path: local_path,
            group,
            context,
        };
        let task_id = self
            .task_manager
            .create_task(dec_id.clone(), source.clone(), DOWNLOAD_CHUNK_TASK, params)
            .await?;

        let mut conn = self.trans_store.create_connection().await?;
        conn.add_task_info(&task_id, &None, TaskStatus::Stopped, vec![(source, dec_id)])
            .await?;

        Ok(task_id)
    }

    pub async fn start_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        self.task_manager.start_task(task_id).await
    }

    pub async fn pause_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        self.task_manager.pause_task(task_id).await
    }

    pub async fn stop_task(&self, task_id: &TaskId) -> BuckyResult<()> {
        self.task_manager.stop_task(task_id).await
    }

    pub async fn get_task_state(&self, task_id: &TaskId) -> BuckyResult<DownloadTaskState> {
        let data = self.task_manager.get_task_detail_status(task_id).await?;
        DownloadTaskState::clone_from_slice(data.as_slice())
    }

    pub async fn remove_task(
        &self,
        source: &DeviceId,
        dec_id: &ObjectId,
        task_id: &TaskId,
    ) -> BuckyResult<()> {
        self.task_manager
            .remove_task(dec_id, source, task_id)
            .await?;
        let mut conn = self.trans_store.create_connection().await?;
        conn.remove_task_info(source, dec_id, task_id).await?;
        Ok(())
    }

    pub async fn get_tasks(
        &self,
        source: &DeviceId,
        dec_id: &ObjectId,
        task_status: Option<TaskStatus>,
        range: Option<(u64, u32)>,
    ) -> BuckyResult<Vec<TransTaskInfo>> {
        let mut conn = self.trans_store.create_connection().await?;
        let task_id_list = conn
            .get_tasks(source, dec_id, &None, task_status, range)
            .await?;

        let list = self
            .task_manager
            .get_tasks_by_task_id(task_id_list.as_slice())
            .await?;
        let mut task_info_list = Vec::new();

        for (task_id, task_type, _, param, _) in list {
            if task_type == DOWNLOAD_CHUNK_TASK {
                let param = DownloadChunkParam::clone_from_slice(param.as_slice())?;
                let local_path = if param.save_path.is_some() {
                    param.save_path.as_ref().unwrap().clone()
                } else {
                    "".to_string()
                };
                task_info_list.push(TransTaskInfo {
                    task_id: task_id.to_string(),
                    context: param.context,
                    object_id: param.chunk_id.object_id(),
                    local_path: PathBuf::from(local_path),
                    device_list: param.device_list,
                });
            } else if task_type == DOWNLOAD_FILE_TASK {
                let param = DownloadFileParam::clone_from_slice(param.as_slice())?;
                let local_path = if param.save_path.is_some() {
                    param.save_path.as_ref().unwrap().clone()
                } else {
                    "".to_string()
                };
                task_info_list.push(TransTaskInfo {
                    task_id: task_id.to_string(),
                    context: param.context,
                    object_id: param.file.desc().calculate_id(),
                    local_path: PathBuf::from(local_path),
                    device_list: param.device_list,
                });
            } else {
                unreachable!("unknown task type: task={}, type={}", task_id, task_type);
            }
        }
        
        Ok(task_info_list)
    }
}
