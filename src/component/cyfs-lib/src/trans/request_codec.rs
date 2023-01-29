use super::output_request::*;
use crate::*;
use cyfs_base::*;

use cyfs_core::TransContext;
use serde_json::{Map, Value};
use std::str::FromStr;

impl JsonCodec<TransGetContextOutputRequest> for TransGetContextOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "context_id",
            self.context_id.as_ref(),
        );
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "context_path",
            self.context_path.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context_id: JsonCodecHelper::decode_option_string_field(obj, "context_id")?,
            context_path: JsonCodecHelper::decode_option_string_field(obj, "context_path")?,
        })
    }
}

impl JsonCodec<TransGetContextInputRequest> for TransGetContextInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "context_id",
            self.context_id.as_ref(),
        );
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "context_path",
            self.context_path.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context_id: JsonCodecHelper::decode_option_string_field(obj, "context_id")?,
            context_path: JsonCodecHelper::decode_option_string_field(obj, "context_path")?,
        })
    }
}

impl JsonCodec<TransPutContextOutputRequest> for TransPutContextOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "context", &self.context.to_hex().unwrap());
        if let Some(access) = &self.access {
            JsonCodecHelper::encode_number_field(&mut obj, "access", access.value());
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let context = TransContext::clone_from_hex(
            JsonCodecHelper::decode_string_field::<String>(obj, "context")?.as_str(),
            &mut Vec::new(),
        )?;
        let access: Option<u32> = JsonCodecHelper::decode_option_int_field(obj, "access")?;
        let access = access.map(|v| AccessString::new(v));

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context,
            access,
        })
    }
}

impl JsonCodec<TransUpdateContextInputRequest> for TransUpdateContextInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "context", &self.context.to_hex().unwrap());
        if let Some(access) = &self.access {
            JsonCodecHelper::encode_number_field(&mut obj, "access", access.value());
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let context = TransContext::clone_from_hex(
            JsonCodecHelper::decode_string_field::<String>(obj, "context")?.as_str(),
            &mut Vec::new(),
        )?;
        let access: Option<u32> = JsonCodecHelper::decode_option_int_field(obj, "access")?;
        let access = access.map(|v| AccessString::new(v));
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            context,
            access,
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

        JsonCodecHelper::encode_option_string_field(&mut obj, "group", self.group.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "context", self.context.as_ref());

        JsonCodecHelper::encode_bool_field(&mut obj, "auto_start", self.auto_start);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            local_path: JsonCodecHelper::decode_string_field(obj, "local_path")?,
            device_list: JsonCodecHelper::decode_str_array_field(obj, "device_list")?,
            group: JsonCodecHelper::decode_option_string_field(obj, "group")?,
            context: JsonCodecHelper::decode_option_string_field(obj, "context")?,
            auto_start: JsonCodecHelper::decode_bool_field(obj, "auto_start")?,
        })
    }
}

impl JsonCodec<TransCreateTaskInputRequest> for TransCreateTaskInputRequest {
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

        JsonCodecHelper::encode_option_string_field(&mut obj, "group", self.group.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "context", self.context.as_ref());

        JsonCodecHelper::encode_bool_field(&mut obj, "auto_start", self.auto_start);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            local_path: JsonCodecHelper::decode_string_field(obj, "local_path")?,
            device_list: JsonCodecHelper::decode_str_array_field(obj, "device_list")?,
            group: JsonCodecHelper::decode_option_string_field(obj, "group")?,
            context: JsonCodecHelper::decode_option_string_field(obj, "context")?,
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

impl JsonCodec<TransControlTaskInputRequest> for TransControlTaskInputRequest {
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

impl JsonCodec<TransGetTaskStateInputRequest> for TransGetTaskStateInputRequest {
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

        if let Some(access) = &self.access {
            JsonCodecHelper::encode_number_field(&mut obj, "access", access.value());
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let access: Option<u32> = JsonCodecHelper::decode_option_int_field(obj, "access")?;
        let access = access.map(|v| AccessString::new(v));

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            owner: JsonCodecHelper::decode_string_field(&obj, "owner")?,
            local_path: JsonCodecHelper::decode_string_field(&obj, "local_path")?,
            chunk_size: JsonCodecHelper::decode_int_field(&obj, "chunk_size")?,
            dirs: JsonCodecHelper::decode_option_array_field(&obj, "dirs")?,
            file_id: JsonCodecHelper::decode_option_string_field(obj, "file_id")?,
            access,
        })
    }
}

impl JsonCodec<TransPublishFileInputRequest> for TransPublishFileInputRequest {
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
        JsonCodecHelper::encode_string_field(&mut obj, "local_path", local_path);
        JsonCodecHelper::encode_number_field(&mut obj, "chunk_size", self.chunk_size);

        JsonCodecHelper::encode_option_string_field(&mut obj, "file_id", self.file_id.as_ref());

        JsonCodecHelper::encode_as_option_list(&mut obj, "dirs", self.dirs.as_ref());

        if let Some(access) = &self.access {
            JsonCodecHelper::encode_number_field(&mut obj, "access", access.value());
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let access: Option<u32> = JsonCodecHelper::decode_option_int_field(obj, "access")?;
        let access = access.map(|v| AccessString::new(v));

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            owner: JsonCodecHelper::decode_string_field(&obj, "owner")?,
            local_path: JsonCodecHelper::decode_string_field(&obj, "local_path")?,
            chunk_size: JsonCodecHelper::decode_int_field(&obj, "chunk_size")?,
            dirs: JsonCodecHelper::decode_option_array_field(&obj, "dirs")?,
            file_id: JsonCodecHelper::decode_option_string_field(obj, "file_id")?,
            access,
        })
    }
}

impl JsonCodecAutoWithSerde for TransPublishFileOutputResponse {}
impl JsonCodecAutoWithSerde for TransPublishFileInputResponse {}

impl JsonCodecAutoWithSerde for TransCreateTaskOutputResponse {}
impl JsonCodecAutoWithSerde for TransCreateTaskInputResponse {}

impl JsonCodec<TransTaskInfo> for TransTaskInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "task_id", &self.task_id);
        if self.context.is_some() {
            JsonCodecHelper::encode_string_field(
                &mut obj,
                "context",
                self.context.as_ref().unwrap(),
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
        Ok(Self {
            task_id: JsonCodecHelper::decode_string_field(obj, "task_id")?,
            context: JsonCodecHelper::decode_option_string_field(obj, "context")?,
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

impl JsonCodec<TransQueryTasksInputResponse> for TransQueryTasksInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_as_list(&mut obj, "task_list", &self.task_list);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let task_list = JsonCodecHelper::decode_array_field(obj, "task_list")?;
        Ok(Self { task_list })
    }
}

impl JsonCodec<TransQueryTasksOutputRequest> for TransQueryTasksOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "task_status",
            self.task_status.as_ref(),
        );

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
            task_status,
            range,
        })
    }
}

impl JsonCodec<TransQueryTasksInputRequest> for TransQueryTasksInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

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

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
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
