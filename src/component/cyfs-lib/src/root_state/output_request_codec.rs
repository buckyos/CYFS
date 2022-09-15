use super::output_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<RootStateOutputRequestCommon> for RootStateOutputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "target_dec_id", self.target_dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<RootStateOutputRequestCommon> {
        Ok(Self {
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            target_dec_id: JsonCodecHelper::decode_option_string_field(obj, "target_dec_id")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

// get_current_root
impl JsonCodec<RootStateGetCurrentRootOutputRequest> for RootStateGetCurrentRootOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "root_type", &self.root_type);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<RootStateGetCurrentRootOutputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            root_type: JsonCodecHelper::decode_string_field(obj, "root_type")?,
        })
    }
}

impl JsonCodec<RootStateGetCurrentRootOutputResponse> for RootStateGetCurrentRootOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "root", &self.root);
        JsonCodecHelper::encode_number_field(&mut obj, "revision", self.revision);
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_root", self.dec_root.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<RootStateGetCurrentRootOutputResponse> {
        Ok(Self {
            root: JsonCodecHelper::decode_string_field(obj, "root")?,
            revision: JsonCodecHelper::decode_int_field(obj, "revision")?,
            dec_root: JsonCodecHelper::decode_option_string_field(obj, "dec_root")?,
        })
    }
}

// create_op_env
impl JsonCodec<Self> for RootStateOpEnvAccess {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "path", &self.path);
        JsonCodecHelper::encode_number_field(&mut obj, "access", self.access as u8);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let access: u8 = JsonCodecHelper::decode_int_field(obj, "access")?;
        
        Ok(Self {
            path: JsonCodecHelper::decode_string_field(obj, "path")?,
            access: AccessPermissions::try_from(access)?,
        })
    }
}

impl JsonCodec<Self> for RootStateCreateOpEnvOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "op_env_type", &self.op_env_type);
        JsonCodecHelper::encode_option_field(&mut obj, "access", self.access.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            op_env_type: JsonCodecHelper::decode_string_field(obj, "op_env_type")?,
            access: JsonCodecHelper::decode_option_field(obj, "access")?,
        })
    }
}

impl JsonCodec<RootStateCreateOpEnvOutputResponse> for RootStateCreateOpEnvOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "sid", &self.sid);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<RootStateCreateOpEnvOutputResponse> {
        Ok(Self {
            sid: JsonCodecHelper::decode_int_field(obj, "sid")?,
        })
    }
}

// op_env requests
impl JsonCodec<Self> for OpEnvOutputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "target_dec_id", self.target_dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);
        JsonCodecHelper::encode_string_field(&mut obj, "sid", &self.sid);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            target_dec_id: JsonCodecHelper::decode_option_string_field(obj, "target_dec_id")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
            sid: JsonCodecHelper::decode_int_field(obj, "sid")?,
        })
    }
}

// load
impl JsonCodec<Self> for OpEnvLoadOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "target", &self.target);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            target: JsonCodecHelper::decode_string_field(obj, "target")?,
        })
    }
}

// load_by_path
impl JsonCodec<Self> for OpEnvLoadByPathOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "path", &self.path);
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            path: JsonCodecHelper::decode_string_field(obj, "path")?,
            common: JsonCodecHelper::decode_field(obj, "common")?,
        })
    }
}

// create_new
impl JsonCodec<Self> for OpEnvCreateNewOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "key", self.key.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "content_type", &self.content_type);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            key: JsonCodecHelper::decode_option_string_field(obj, "key")?,
            content_type: JsonCodecHelper::decode_string_field(obj, "content_type")?,
        })
    }
}

// lock
impl JsonCodec<Self> for OpEnvLockOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_str_array_field(&mut obj, "path_list", &self.path_list);
        JsonCodecHelper::encode_number_field(
            &mut obj,
            "duration_in_millsecs",
            self.duration_in_millsecs,
        );
        JsonCodecHelper::encode_bool_field(
            &mut obj,
            "try_lock",
            self.try_lock,
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path_list: JsonCodecHelper::decode_str_array_field(obj, "path_list")?,
            duration_in_millsecs: JsonCodecHelper::decode_int_field(obj, "duration_in_millsecs")?,
            try_lock: JsonCodecHelper::decode_bool_field(obj, "try_lock")?,
        })
    }
}

// commit
impl JsonCodec<Self> for OpEnvCommitOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(&mut obj, "op_type", self.op_type.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            op_type: JsonCodecHelper::decode_option_string_field(obj, "op_type")?,
        })
    }
}
impl JsonCodec<Self> for OpEnvCommitOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "root", &self.root);
        JsonCodecHelper::encode_string_field(&mut obj, "dec_root", &self.dec_root);
        JsonCodecHelper::encode_number_field(&mut obj, "revision", self.revision);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            root: JsonCodecHelper::decode_string_field(obj, "root")?,
            dec_root: JsonCodecHelper::decode_string_field(obj, "dec_root")?,
            revision: JsonCodecHelper::decode_int_field(obj, "revision")?,
        })
    }
}

// abort
impl JsonCodec<Self> for OpEnvAbortOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
        })
    }
}

// metadata
impl JsonCodec<Self> for OpEnvMetadataOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
        })
    }
}
impl JsonCodec<Self> for OpEnvMetadataOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "content_mode", &self.content_mode);
        JsonCodecHelper::encode_string_field(&mut obj, "content_type", &self.content_type);
        JsonCodecHelper::encode_string_field(&mut obj, "count", &self.count);
        JsonCodecHelper::encode_string_field(&mut obj, "size", &self.size);
        JsonCodecHelper::encode_string_field(&mut obj, "depth", &self.depth);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            content_mode: JsonCodecHelper::decode_string_field(obj, "content_mode")?,
            content_type: JsonCodecHelper::decode_string_field(obj, "content_type")?,
            count: JsonCodecHelper::decode_int_field(obj, "count")?,
            size: JsonCodecHelper::decode_int_field(obj, "size")?,
            depth: JsonCodecHelper::decode_int_field(obj, "depth")?,
        })
    }
}

// get_by_key
impl JsonCodec<Self> for OpEnvGetByKeyOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "key", &self.key);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            common: JsonCodecHelper::decode_field(obj, "common")?,
            key: JsonCodecHelper::decode_string_field(obj, "key")?,
        })
    }
}

impl JsonCodec<OpEnvGetByKeyOutputResponse> for OpEnvGetByKeyOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "value", self.value.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<OpEnvGetByKeyOutputResponse> {
        Ok(Self {
            value: JsonCodecHelper::decode_option_string_field(obj, "value")?,
        })
    }
}

// insert_with_key
impl JsonCodec<Self> for OpEnvInsertWithKeyOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "key", &self.key);
        JsonCodecHelper::encode_string_field(&mut obj, "value", &self.value);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            key: JsonCodecHelper::decode_string_field(obj, "key")?,
            value: JsonCodecHelper::decode_string_field(obj, "value")?,
        })
    }
}

// set_with_key
impl JsonCodec<Self> for OpEnvSetWithKeyOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "key", &self.key);
        JsonCodecHelper::encode_string_field(&mut obj, "value", &self.value);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "prev_value",
            self.prev_value.as_ref(),
        );
        JsonCodecHelper::encode_bool_field(&mut obj, "auto_insert", self.auto_insert);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            key: JsonCodecHelper::decode_string_field(obj, "key")?,
            value: JsonCodecHelper::decode_string_field(obj, "value")?,
            prev_value: JsonCodecHelper::decode_option_string_field(obj, "prev_value")?,
            auto_insert: JsonCodecHelper::decode_bool_field(obj, "auto_insert")?,
        })
    }
}

impl JsonCodec<OpEnvSetWithKeyOutputResponse> for OpEnvSetWithKeyOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "prev_value",
            self.prev_value.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<OpEnvSetWithKeyOutputResponse> {
        Ok(Self {
            prev_value: JsonCodecHelper::decode_option_string_field(obj, "prev_value")?,
        })
    }
}

// remove_with_key
impl JsonCodec<Self> for OpEnvRemoveWithKeyOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "key", &self.key);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "prev_value",
            self.prev_value.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            key: JsonCodecHelper::decode_string_field(obj, "key")?,
            prev_value: JsonCodecHelper::decode_option_string_field(obj, "prev_value")?,
        })
    }
}

impl JsonCodec<OpEnvRemoveWithKeyOutputResponse> for OpEnvRemoveWithKeyOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "value", self.value.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<OpEnvRemoveWithKeyOutputResponse> {
        Ok(Self {
            value: JsonCodecHelper::decode_option_string_field(obj, "value")?,
        })
    }
}

// set requests
impl JsonCodec<Self> for OpEnvSetOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_option_string_field(&mut obj, "path", self.path.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "value", &self.value);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            path: JsonCodecHelper::decode_option_string_field(obj, "path")?,
            value: JsonCodecHelper::decode_string_field(obj, "value")?,
        })
    }
}

impl JsonCodec<OpEnvSetOutputResponse> for OpEnvSetOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("result".to_owned(), Value::Bool(self.result));

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<OpEnvSetOutputResponse> {
        Ok(Self {
            result: JsonCodecHelper::decode_bool_field(obj, "result")?,
        })
    }
}

// next
impl JsonCodec<Self> for OpEnvNextOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_number_field(&mut obj, "step", self.step);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            step: JsonCodecHelper::decode_int_field(obj, "step")?,
        })
    }
}


impl JsonCodec<OpEnvNextOutputResponse> for OpEnvNextOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_as_list(&mut obj, "result", &self.list);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            list: JsonCodecHelper::decode_array_field(obj, "result")?,
        })
    }
}
