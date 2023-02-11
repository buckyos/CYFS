use crate::trans_api::{TransStore};
use crate::NamedDataComponents;
use cyfs_base::*;
use cyfs_bdt::{self, StackGuard};
use cyfs_task_manager::*;
use super::download_task::*;

use std::sync::Arc;


#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::super::trans_proto::DownloadFileParam)]
pub struct DownloadFileParam {
    pub dec_id: ObjectId,
    pub file: File,
    pub device_list: Vec<DeviceId>,
    pub referer: String,
    pub save_path: Option<String>,
    pub group: Option<String>,
    pub context: Option<String>,
}

impl ProtobufTransform<super::super::trans_proto::DownloadFileParam> for DownloadFileParam {
    fn transform(
        value: crate::trans_api::local::trans_proto::DownloadFileParam,
    ) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(DeviceId::clone_from_slice(item.as_slice())?);
        }
        Ok(Self {
            dec_id: ObjectId::clone_from_slice(&value.dec_id)?,
            file: File::clone_from_slice(value.file.as_slice())?,
            device_list,
            referer: value.referer,
            save_path: value.save_path,
            context: value.context,
            group: value.group,
        })
    }
}

impl ProtobufTransform<&DownloadFileParam> for super::super::trans_proto::DownloadFileParam {
    fn transform(value: &DownloadFileParam) -> BuckyResult<Self> {
        let mut device_list = Vec::new();
        for item in value.device_list.iter() {
            device_list.push(item.to_vec()?);
        }
        Ok(Self {
            dec_id: value.dec_id.to_vec()?,
            file: value.file.to_vec()?,
            device_list,
            referer: value.referer.clone(),
            save_path: value.save_path.clone(),
            context: value.context.clone(),
            group: value.group.clone(),
        })
    }
}


pub(crate) struct DownloadFileTaskFactory {
    named_data_components: NamedDataComponents,
    stack: StackGuard,
    trans_store: Arc<TransStore>,
}

impl DownloadFileTaskFactory {
    pub fn new(
        stack: StackGuard,
        named_data_components: NamedDataComponents,
        trans_store: Arc<TransStore>,
    ) -> Self {
        Self {
            stack,
            named_data_components,
            trans_store,
        }
    }
}

#[async_trait::async_trait]
impl TaskFactory for DownloadFileTaskFactory {
    fn get_task_type(&self) -> TaskType {
        DOWNLOAD_FILE_TASK
    }

    async fn create(&self, params: &[u8]) -> BuckyResult<Box<dyn Task>> {
        let params = DownloadFileParam::clone_from_slice(params)?;
        let params = DownloadFileTaskParams::new_file(params);

        let task = DownloadFileTask::new(
            self.named_data_components.clone(),
            self.stack.clone(),
            self.trans_store.clone(),
            DownloadFileTaskStatus::new(),
            params,
        );
        Ok(Box::new(task))
    }

    async fn restore(
        &self,
        _task_status: TaskStatus,
        params: &[u8],
        data: &[u8],
    ) -> BuckyResult<Box<dyn Task>> {
        let params = DownloadFileParam::clone_from_slice(params)?;
        let params = DownloadFileTaskParams::new_file(params);

        let status = DownloadFileTaskStatus::load(data)?;

        let task = DownloadFileTask::new(
            self.named_data_components.clone(),
            self.stack.clone(),
            self.trans_store.clone(),
            status,
            params,
        );
        Ok(Box::new(task))
    }
}
