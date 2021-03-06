use super::output_request::*;
use crate::*;
use cyfs_base::*;

use cyfs_core::TransContext;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::str::FromStr;

impl JsonCodec<TransGetContextOutputRequest> for TransGetContextOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "context_name", &self.context_name);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransGetContextOutputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context_name: JsonCodecHelper::decode_string_field(obj, "context_name")?,
        })
    }
}

impl JsonCodec<TransPutContextOutputRequest> for TransPutContextOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "context", &self.context.to_hex().unwrap());
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransPutContextOutputRequest> {
        let context = TransContext::clone_from_hex(
            JsonCodecHelper::decode_string_field::<String>(obj, "context")?.as_str(),
            &mut Vec::new(),
        )?;
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context,
        })
    }
}
impl JsonCodec<TransCreateTaskOutputRequest> for TransCreateTaskOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);

        let local_path = self.local_path.to_str().unwrap_or_else(|| {
            error!(
                "invalid utf-8 local_path value: {}",
                self.local_path.display()
            );
            ""
        });
        obj.insert(
            "local_path".to_owned(),
            Value::String(local_path.to_string()),
        );

        JsonCodecHelper::encode_str_array_field(&mut obj, "device_list", &self.device_list);

        if self.context_id.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "context_id",
                self.context_id.as_ref().unwrap(),
            );
        }
        JsonCodecHelper::encode_bool_field(&mut obj, "auto_start", self.auto_start);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let context_id = match obj.get("context_id") {
            Some(context_id) => Some(JsonCodecHelper::decode_from_string(context_id)?),
            None => None,
        };

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            local_path: JsonCodecHelper::decode_string_field(obj, "local_path")?,
            device_list: JsonCodecHelper::decode_str_array_field(obj, "device_list")?,
            context_id,
            auto_start: JsonCodecHelper::decode_bool_field(obj, "auto_start")?,
        })
    }
}

impl JsonCodec<TransControlTaskOutputRequest> for TransControlTaskOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "task_id", &self.task_id);
        JsonCodecHelper::encode_string_field(&mut obj, "action", &self.action);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            task_id: JsonCodecHelper::decode_string_field(obj, "task_id")?,
            action: JsonCodecHelper::decode_string_field(obj, "action")?,
        })
    }
}

impl JsonCodec<TransGetTaskStateOutputRequest> for TransGetTaskStateOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        obj.insert("task_id".to_owned(), Value::String(self.task_id.clone()));
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            task_id: JsonCodecHelper::decode_string_field(obj, "task_id")?,
        })
    }
}


impl JsonCodec<TransPublishFileOutputRequest> for TransPublishFileOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        obj.insert("owner".to_owned(), Value::String(self.owner.to_string()));

        let local_path = self.local_path.to_str().unwrap_or_else(|| {
            error!(
                "invalid utf-8 local_path value: {}",
                self.local_path.display()
            );
            ""
        });
        obj.insert(
            "local_path".to_owned(),
            Value::String(local_path.to_string()),
        );

        obj.insert(
            "chunk_size".to_owned(),
            Value::String(self.chunk_size.to_string()),
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "file_id", self.file_id.as_ref());

        if let Some(dirs) = &self.dirs {
            let node = JsonCodecHelper::encode_to_array(dirs);
            obj.insert("dirs".to_owned(), node);
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut owner: Option<ObjectId> = None;
        let mut local_path: Option<PathBuf> = None;
        let mut chunk_size: Option<u32> = None;
        let mut dirs: Option<Vec<FileDirRef>> = None;

        for (k, v) in obj {
            match k.as_str() {
                "owner" => {
                    owner = Some(JsonCodecHelper::decode_from_string(v)?);
                }

                "local_path" => {
                    local_path = Some(JsonCodecHelper::decode_from_string(v)?);
                }

                "chunk_size" => {
                    chunk_size = Some(JsonCodecHelper::decode_to_int(v)?);
                }

                "dirs" => {
                    if !JsonCodecHelper::is_none_node(v) {
                        dirs = Some(JsonCodecHelper::decode_from_array(v)?);
                    }
                }

                u @ _ => {
                    warn!("unknown TransAddFileRequest field: {}", u);
                }
            }
        }

        if owner.is_none() || local_path.is_none() || chunk_size.is_none() {
            error!(
                "owner/local_path/chunk_size/start_upload/user_id field is missing! {:?}",
                obj
            );
            return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
        }

        let file_id = JsonCodecHelper::decode_option_string_field(obj, "file_id")?;

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            owner: owner.unwrap(),
            local_path: local_path.unwrap(),
            chunk_size: chunk_size.unwrap(),
            dirs,
            file_id,
        })
    }
}

impl JsonCodec<TransPublishFileOutputResponse> for TransPublishFileOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert(
            "file_id".to_owned(),
            Value::String(self.file_id.to_string()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let file_id = JsonCodecHelper::decode_string_field(obj, "file_id")?;

        Ok(Self { file_id })
    }
}

impl JsonCodec<TransCreateTaskOutputResponse> for TransCreateTaskOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        obj.insert("task_id".to_owned(), Value::String(self.task_id.clone()));
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransCreateTaskOutputResponse> {
        let task_id = JsonCodecHelper::decode_string_field(obj, "task_id")?;
        Ok(Self { task_id })
    }
}

impl JsonCodec<TransTaskInfo> for TransTaskInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "task_id", &self.task_id);
        if self.context_id.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "context_id",
                self.context_id.as_ref().unwrap(),
            );
        }
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "local_path",
            &self.local_path.to_string_lossy().to_string(),
        );
        JsonCodecHelper::encode_str_array_field(&mut obj, "device_list", &self.device_list);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransTaskInfo> {
        let context_id = match obj.get("context_id") {
            Some(context_id) => Some(JsonCodecHelper::decode_from_string(context_id)?),
            None => None,
        };
        Ok(Self {
            task_id: JsonCodecHelper::decode_string_field(obj, "task_id")?,
            context_id,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            local_path: JsonCodecHelper::decode_string_field(obj, "local_path")?,
            device_list: JsonCodecHelper::decode_str_array_field(obj, "device_list")?,
        })
    }
}

impl JsonCodec<TransQueryTasksOutputResponse> for TransQueryTasksOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_as_list(&mut obj, "task_list", &self.task_list);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransQueryTasksOutputResponse> {
        let task_list = JsonCodecHelper::decode_array_field(obj, "task_list")?;
        Ok(TransQueryTasksOutputResponse { task_list })
    }
}

impl JsonCodec<TransQueryTasksOutputRequest> for TransQueryTasksOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        if self.context_id.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "context_id",
                self.context_id.as_ref().unwrap(),
            );
        }
        if self.task_status.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "task_status",
                self.task_status.as_ref().unwrap(),
            );
        }
        if self.range.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "offset",
                &self.range.as_ref().unwrap().0,
            );
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "length",
                &self.range.as_ref().unwrap().1,
            );
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<TransQueryTasksOutputRequest> {
        let context_id = match obj.get("context_id") {
            Some(context_id) => Some(JsonCodecHelper::decode_from_string(context_id)?),
            None => None,
        };

        let task_status = match obj.get("task_status") {
            Some(task_status) => Some(JsonCodecHelper::decode_from_string(task_status)?),
            None => None,
        };

        let offset = match obj.get("offset") {
            Some(offset) => Some(JsonCodecHelper::decode_from_string(offset)?),
            None => None,
        };

        let length = match obj.get("length") {
            Some(length) => Some(JsonCodecHelper::decode_from_string(length)?),
            None => None,
        };

        let range = if offset.is_some() && length.is_some() {
            Some((offset.unwrap(), length.unwrap()))
        } else {
            None
        };

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context_id,
            task_status,
            range,
        })
    }
}

impl ToString for TransTaskStatus {
    fn to_string(&self) -> String {
        match self {
            TransTaskStatus::Stopped => "Stopped".to_string(),
            TransTaskStatus::Failed => "Failed".to_string(),
            TransTaskStatus::Running => "Running".to_string(),
            TransTaskStatus::Finished => "Finished".to_string(),
        }
    }
}

impl FromStr for TransTaskStatus {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Stopped" => Ok(TransTaskStatus::Stopped),
            "Failed" => Ok(TransTaskStatus::Failed),
            "Running" => Ok(TransTaskStatus::Running),
            "Finished" => Ok(TransTaskStatus::Finished),
            _ => {
                let msg = format!("unknown TransTaskStatus {}", s);
                error!("{}", msg.as_str());
                Err(BuckyError::new(BuckyErrorCode::InvalidInput, msg))
            }
        }
    }
}
