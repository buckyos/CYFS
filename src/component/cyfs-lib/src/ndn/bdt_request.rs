use crate::*;
use cyfs_base::*;

use serde_json::{Map, Value};
use std::fmt;

// 用以bdt层的权限控制交换信息
pub struct BdtDataRefererInfo {
    // refer target: maybe owner of object; or dsg contract etc.
    pub target: Option<ObjectId>, 
    pub object_id: ObjectId,
    pub inner_path: Option<String>,

    // source-dec-id
    pub dec_id: Option<ObjectId>,

    // target-dec-id and req-path, etc
    pub req_path: Option<String>,
    
    pub referer_object: Vec<NDNDataRefererObject>,
    pub flags: u32,
}

impl fmt::Display for BdtDataRefererInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }
        write!(f, "object_id: {:?}", self.object_id)?;
        if let Some(inner_path) = &self.inner_path {
            write!(f, ", inner_path: {}", inner_path)?;
        }

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }
        
        write!(f, ", req_path: {:?}", self.req_path)?;
        if !self.referer_object.is_empty() {
            write!(f, ", referer_object: {:?}", self.referer_object)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

impl JsonCodec<BdtDataRefererInfo> for BdtDataRefererInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_option_string_field(&mut obj, "inner_path", self.inner_path.as_ref());

        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());

        if self.referer_object.len() > 0 {
            JsonCodecHelper::encode_str_array_field(
                &mut obj,
                "referer_object",
                &self.referer_object,
            );
        }

        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<BdtDataRefererInfo> {
        Ok(Self {
            target: JsonCodecHelper::decode_option_string_field(obj,"target")?,
            object_id: JsonCodecHelper::decode_string_field(obj,"object_id")?,
            inner_path: JsonCodecHelper::decode_option_string_field(obj,"inner_path")?,
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            referer_object: JsonCodecHelper::decode_str_array_field(obj, "referer_object")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}
