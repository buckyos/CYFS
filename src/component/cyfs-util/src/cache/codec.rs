use super::named_data_cache::FileDirRef;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<FileDirRef> for FileDirRef {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        obj.insert("dir_id".to_owned(), Value::String(self.dir_id.to_string()));
        obj.insert(
            "inner_path".to_owned(),
            Value::String(self.inner_path.clone()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let dir_id = JsonCodecHelper::decode_string_field(obj, "dir_id")?;
        let inner_path = JsonCodecHelper::decode_string_field(obj, "inner_path")?;

        Ok(Self { dir_id, inner_path })
    }
}