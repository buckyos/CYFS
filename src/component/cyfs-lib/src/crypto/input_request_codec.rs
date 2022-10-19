use super::input_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<CryptoInputRequestCommon> for CryptoInputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "source", &self.source);
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoInputRequestCommon> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            source: JsonCodecHelper::decode_field(obj, "source")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

// sign_object
impl JsonCodec<CryptoSignObjectInputRequest> for CryptoSignObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoSignObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<CryptoSignObjectInputResponse> for CryptoSignObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "result", &self.result);
        JsonCodecHelper::encode_option_field(&mut obj, "object", self.object.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoSignObjectInputResponse> {
        Ok(Self {
            result: JsonCodecHelper::decode_string_field(obj, "result")?,
            object: JsonCodecHelper::decode_option_field(obj, "object")?,
        })
    }
}

// verify_object
impl JsonCodec<CryptoVerifyObjectInputRequest> for CryptoVerifyObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_string_field(&mut obj, "sign_type", &self.sign_type);

        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);

        JsonCodecHelper::encode_field(&mut obj, "sign_object", &self.sign_object);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoVerifyObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            sign_type: JsonCodecHelper::decode_string_field(obj, "sign_type")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
            sign_object: JsonCodecHelper::decode_field(obj, "sign_object")?,
        })
    }
}

// encrypt_data
impl JsonCodec<Self> for CryptoEncryptDataInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_string_field(&mut obj, "encrypt_type", &self.encrypt_type);

        if let Some(data) = &self.data {
            JsonCodecHelper::encode_string_field_2(&mut obj, "data", hex::encode(data));
        }

        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let s: Option<String> = JsonCodecHelper::decode_option_string_field(obj, "data")?;
        let data = match s {
            Some(s) => Some(hex::decode(&s).map_err(|e| {
                let msg = format!("invalid encrypt data string: {}, {}", s, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?),
            None => None,
        };

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            encrypt_type: JsonCodecHelper::decode_string_field(obj, "encrypt_type")?,
            data,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<Self> for CryptoEncryptDataInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "aes_key", self.aes_key.as_ref());

        JsonCodecHelper::encode_string_field_2(&mut obj, "data", hex::encode(&self.result));

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let s: String = JsonCodecHelper::decode_string_field(obj, "data")?;
        let result = hex::decode(&s).map_err(|e| {
            let msg = format!("invalid encrypt data result string: {:?}, {}", s, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(Self {
            aes_key: JsonCodecHelper::decode_option_string_field(obj, "aes_key")?,
            result,
        })
    }
}


// decrypt_data
impl JsonCodec<Self> for CryptoDecryptDataInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_string_field(&mut obj, "decrypt_type", &self.decrypt_type);

        JsonCodecHelper::encode_string_field_2(&mut obj, "data", hex::encode(&self.data));

        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let s: String = JsonCodecHelper::decode_string_field(obj, "data")?;
        let data = hex::decode(&s).map_err(|e| {
                let msg = format!("invalid encrypt data string: {}, {}", s, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            decrypt_type: JsonCodecHelper::decode_string_field(obj, "decrypt_type")?,
            data,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<Self> for CryptoDecryptDataInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "result", &self.result);
        JsonCodecHelper::encode_string_field_2(&mut obj, "data", hex::encode(&self.data));

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let s: String = JsonCodecHelper::decode_string_field(obj, "data")?;
        let data = hex::decode(&s).map_err(|e| {
            let msg = format!("invalid decrypt data result string: {}, {}", s, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(Self {
            result: JsonCodecHelper::decode_string_field(obj, "result")?,
            data,
        })
    }
}