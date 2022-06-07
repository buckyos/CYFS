use super::output_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<VerifySigns> for VerifySigns {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        if let Some(signs) = &self.body_signs {
            JsonCodecHelper::encode_number_array_field(&mut obj, "body_signs", &signs);
        }
        if let Some(signs) = &self.desc_signs {
            JsonCodecHelper::encode_number_array_field(&mut obj, "desc_signs", &signs);
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            desc_signs: JsonCodecHelper::decode_option_int_array_field(&obj, "desc_signs")?,
            body_signs: JsonCodecHelper::decode_option_int_array_field(&obj, "body_signs")?,
        })
    }
}

impl JsonCodec<VerifySignResult> for VerifySignResult {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("index".to_owned(), Value::String(self.index.to_string()));

        obj.insert("valid".to_owned(), Value::Bool(self.valid));

        obj.insert(
            "sign_object_id".to_owned(),
            Value::String(self.sign_object_id.to_string()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut index: Option<u8> = None;
        let mut valid: Option<bool> = None;
        let mut sign_object_id: Option<ObjectId> = None;

        for (k, v) in obj {
            match k.as_str() {
                "index" => {
                    index = Some(JsonCodecHelper::decode_from_string(v)?);
                }

                "valid" => {
                    valid = Some(JsonCodecHelper::decode_from_boolean(v)?);
                }

                "sign_object_id" => {
                    sign_object_id = Some(JsonCodecHelper::decode_from_string(v)?);
                }

                u @ _ => {
                    warn!("unknown verify result field: {}", u);
                }
            }
        }

        if index.is_none() || valid.is_none() || sign_object_id.is_none() {
            let msg = format!(
                "verify result item field missing: index/valid/sign_object_id, value={:?}",
                obj
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self {
            index: index.unwrap(),
            valid: valid.unwrap(),
            sign_object_id: sign_object_id.unwrap(),
        })
    }
}

impl JsonCodec<VerifyObjectResult> for VerifyObjectResult {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("valid".to_owned(), Value::Bool(self.valid));

        JsonCodecHelper::encode_as_list(&mut obj, "desc_signs", &self.desc_signs);
        JsonCodecHelper::encode_as_list(&mut obj, "body_signs", &self.body_signs);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut valid: Option<bool> = None;
        let mut desc_signs = Vec::new();
        let mut body_signs = Vec::new();

        for (k, v) in obj {
            match k.as_str() {
                "valid" => {
                    valid = Some(JsonCodecHelper::decode_from_boolean(v)?);
                }

                "desc_signs" => {
                    desc_signs = JsonCodecHelper::decode_from_array(v)?;
                }

                "body_signs" => {
                    body_signs = JsonCodecHelper::decode_from_array(v)?;
                }

                u @ _ => {
                    warn!("unknown verify result field: {}", u);
                }
            }
        }

        if valid.is_none() {
            let msg = format!("verify result item field missing: valid, value={:?}", obj);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self {
            valid: valid.unwrap(),
            desc_signs,
            body_signs,
        })
    }
}

impl JsonCodec<CryptoVerifyObjectOutputResponse> for CryptoVerifyObjectOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "result", &self.result);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            result: JsonCodecHelper::decode_field(&obj, "result")?,
        })
    }
}


// verify_object
impl JsonCodec<VerifyObjectType> for VerifyObjectType {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "type", self);

        match &self {
            VerifyObjectType::Owner | VerifyObjectType::Own => {}
            VerifyObjectType::Object(sign_object) => {
                JsonCodecHelper::encode_field(&mut obj, "sign_object", sign_object);
            }
            VerifyObjectType::Sign(verify_signs) => {
                JsonCodecHelper::encode_field(&mut obj, "verify_signs", verify_signs);
            }
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<VerifyObjectType> {
        let sign_object_type: String = JsonCodecHelper::decode_string_field(obj, "type")?;
        let ret = match sign_object_type.as_str() {
            "owner" => VerifyObjectType::Owner,
            "own" => VerifyObjectType::Own,
            "object" => {
                let sign_object = JsonCodecHelper::decode_field(obj, "sign_object")?;
                VerifyObjectType::Object(sign_object)
            }
            "sign" => {
                let verify_signs = JsonCodecHelper::decode_field(obj, "verify_signs")?;
                VerifyObjectType::Sign(verify_signs)
            }
            _ => {
                let msg = format!("unknown VerifyObjectType type: {}", sign_object_type);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}
