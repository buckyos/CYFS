use crate::*;
use serde_json::{Map, Value};

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::str::FromStr;

pub trait JsonCodec<T> {
    fn encode_json(&self) -> Map<String, Value> {
        unimplemented!();
    }
    fn decode_json(_obj: &Map<String, Value>) -> BuckyResult<T> {
        unimplemented!();
    }

    fn encode_string(&self) -> String {
        self.encode_value().to_string()
    }

    fn decode_string(value: &str) -> BuckyResult<T> {
        let value: Value = serde_json::from_str(value).map_err(|e| {
            error!("invalid json buf str: {} {}", value, e);
            BuckyError::from(BuckyErrorCode::InvalidFormat)
        })?;

        Self::decode_value(&value)
    }

    fn decode_value(value: &Value) -> BuckyResult<T> {
        let obj = value.as_object().ok_or_else(|| {
            let msg = format!("invalid json object format: {}", value);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Self::decode_json(obj)
    }

    fn encode_value(&self) -> Value {
        JsonCodecHelper::encode_value(self.encode_json())
    }
}

pub trait JsonCodecAutoWithSerde {}

impl<T> JsonCodec<T> for T
where
    T: Serialize + for<'d> Deserialize<'d> + JsonCodecAutoWithSerde,
{
    fn encode_json(&self) -> Map<String, Value> {
        unimplemented!();
    }

    fn decode_json(_obj: &Map<String, Value>) -> BuckyResult<T> {
        unimplemented!();
    }

    fn encode_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn decode_value(obj: &Value) -> BuckyResult<T> {
        T::deserialize(obj).map_err(|e| {
            let msg = format!("decode from json error! {:?}, {}", obj, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }
}

pub struct JsonCodecHelper;

impl JsonCodecHelper {
    // None在js里面会编码成{}，如果不是使用serde_json的解码，而是手工操作，这里需要对这种情况加以判断
    pub fn is_none_node(node: &Value) -> bool {
        match node.is_object() {
            true => node.as_object().unwrap().is_empty(),
            false => false,
        }
    }

    pub fn encode_string(obj: Map<String, Value>) -> String {
        Self::encode_value(obj).to_string()
    }

    pub fn encode_value(obj: Map<String, Value>) -> Value {
        Value::Object(obj)
    }

    pub fn encode_string_field<T: ?Sized>(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: &T,
    ) where
        T: ToString,
    {
        obj.insert(key.to_string(), Value::String(value.to_string()));
    }

    pub fn encode_string_field_2(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: impl ToString,
    ) {
        obj.insert(key.to_string(), Value::String(value.to_string()));
    }

    pub fn encode_option_string_field<T: ?Sized>(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: Option<&T>,
    ) where
        T: ToString,
    {
        if let Some(value) = value {
            obj.insert(key.to_string(), Value::String(value.to_string()));
        }
    }

    pub fn encode_number_field<T>(obj: &mut Map<String, Value>, key: impl ToString, value: T)
    where
        T: Into<serde_json::Number>,
    {
        obj.insert(key.to_string(), Value::Number(value.into()));
    }

    pub fn encode_bool_field(obj: &mut Map<String, Value>, key: impl ToString, value: bool) {
        obj.insert(key.to_string(), Value::Bool(value));
    }

    pub fn encode_option_number_field<T>(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: Option<T>,
    ) where
        T: Into<serde_json::Number>,
    {
        if let Some(value) = value {
            obj.insert(key.to_string(), Value::Number(value.into()));
        }
    }

    pub fn decode_string_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_from_string(v)
    }

    pub fn decode_option_string_field<T>(
        obj: &Map<String, Value>,
        key: &str,
    ) -> BuckyResult<Option<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match obj.get(key) {
            Some(v) => {
                let obj = Self::decode_from_string(v)?;
                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    pub fn decode_serde_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<T>
    where
    T: for<'a> serde::de::Deserialize<'a>,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        <T as Deserialize>::deserialize(v).map_err(|e| {
            let msg = format!("decode field with serde failed!: key={}, value={:?}, {}", key, v, e);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })
    }

    pub fn decode_option_serde_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Option<T>>
    where
    T: for<'a> serde::de::Deserialize<'a>,
    {
        match obj.get(key) {
            Some(v) => {
                let ret = <T as Deserialize>::deserialize(v).map_err(|e| {
                    let msg = format!("decode field with serde failed!: key={}, value={:?}, {}", key, v, e);
                    warn!("{}", msg);
        
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;
                Ok(Some(ret))
            }
            None => Ok(None)
        }
    }

    pub fn decode_int_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<T>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_to_int(v)
    }

    pub fn decode_option_int_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Option<T>>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        match obj.get(key) {
            Some(v) => {
                let obj = Self::decode_to_int(v)?;
                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    pub fn decode_to_int<T>(v: &Value) -> BuckyResult<T>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        if v.is_string() {
            let v = T::from_str(v.as_str().unwrap()).map_err(|e| {
                let msg = format!(
                    "parse json string to int error: value={}, {}",
                    v.as_str().unwrap(),
                    e
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            Ok(v)
        } else if v.is_number() {
            if v.is_i64() {
                let v = T::try_from(v.as_i64().unwrap()).map_err(|e| {
                    let msg = format!(
                        "parse json number to int error: value={}, {}",
                        v.as_i64().unwrap(),
                        e
                    );
                    warn!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;
                Ok(v)
            } else if v.is_u64() {
                let v = T::try_from(v.as_u64().unwrap()).map_err(|e| {
                    let msg = format!(
                        "parse json number to int error: value={}, {}",
                        v.as_u64().unwrap(),
                        e
                    );
                    warn!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;
                Ok(v)
            } else {
                let msg = format!(
                    "parse json float number to int error: value={}",
                    v.as_u64().unwrap(),
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        } else {
            let msg = format!("invalid json field, except string or number: {}", v);
            warn!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        }
    }

    pub fn decode_from_serde_string<T>(v: &Value) -> BuckyResult<T>
    where
        T: for<'a> serde::de::Deserialize<'a>,
    {
        if !v.is_string() {
            let msg = format!("invalid json field, except string: {}", v);
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let v = serde_json::from_str(v.as_str().unwrap()).map_err(|e| {
            let msg = format!(
                "parse json string error: value={}, {}",
                v.as_str().unwrap(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(v)
    }

    pub fn decode_from_string<T>(v: &Value) -> BuckyResult<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        if !v.is_string() {
            let msg = format!("invalid json field, except string: {}", v);
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let v = T::from_str(v.as_str().unwrap()).map_err(|e| {
            let msg = format!(
                "parse json string error: value={}, {}",
                v.as_str().unwrap(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(v)
    }

    pub fn decode_bool_field(obj: &Map<String, Value>, key: &str) -> BuckyResult<bool> {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_from_boolean(v)
    }

    pub fn decode_object_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<T>
    where
        T: for<'de> RawFrom<'de, T>,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_object_from_string(v)
    }

    pub fn decode_object_from_string<T>(v: &Value) -> BuckyResult<T>
    where
        T: for<'de> RawFrom<'de, T>,
        //<T as FromStr>::Err: std::fmt::Display,
    {
        if !v.is_string() {
            let msg = format!("invalid json field, except string: {}", v);
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let buf = hex::decode(v.as_str().unwrap()).map_err(|e| {
            let msg = format!(
                "parse object hex string error: value={}, {}",
                v.as_str().unwrap(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let v = T::clone_from_slice(&buf).map_err(|e| {
            let msg = format!(
                "decode object from hex buf error: value={}, {}",
                v.as_str().unwrap(),
                e
            );
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(v)
    }

    pub fn decode_from_boolean(v: &Value) -> BuckyResult<bool> {
        let v = v.as_bool().ok_or_else(|| {
            let msg = format!("invalid json field, except bool: {}", v);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(v)
    }

    /// number array
    pub fn encode_number_array_field<T>(obj: &mut Map<String, Value>, key: &str, value: &Vec<T>)
    where
        T: Into<serde_json::Number> + Copy,
    {
        obj.insert(key.to_owned(), Self::encode_to_number_array(value));
    }

    pub fn encode_to_number_array<T>(list: &Vec<T>) -> Value
    where
        T: Into<serde_json::Number> + Copy,
    {
        let mut result = Vec::new();
        for item in list {
            result.push(Value::Number((*item).into()));
        }

        Value::Array(result)
    }

    pub fn decode_int_array_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Vec<T>>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        match obj.get(key) {
            Some(v) => Self::decode_from_int_array(v),
            None => Ok(vec![]),
        }
    }

    pub fn decode_option_int_array_field<T>(
        obj: &Map<String, Value>,
        key: &str,
    ) -> BuckyResult<Option<Vec<T>>>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        let ret = match obj.get(key) {
            Some(v) => Some(Self::decode_from_int_array(v)?),
            None => None,
        };

        Ok(ret)
    }

    pub fn decode_from_int_array<T>(v: &Value) -> BuckyResult<Vec<T>>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        let list = v.as_array().ok_or_else(|| {
            let msg = format!("invalid json field, except array: {}", v);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let mut result = Vec::new();
        for item in list {
            let item = Self::decode_to_int(item)?;
            result.push(item);
        }

        Ok(result)
    }

    /// string array
    pub fn encode_str_array_field<T>(obj: &mut Map<String, Value>, key: &str, value: &Vec<T>)
    where
        T: ToString,
    {
        obj.insert(key.to_owned(), Self::encode_to_str_array(value));
    }

    pub fn encode_option_str_array_field<T>(
        obj: &mut Map<String, Value>,
        key: &str,
        value: Option<&Vec<T>>,
    ) where
        T: ToString,
    {
        if let Some(value) = value {
            obj.insert(key.to_owned(), Self::encode_to_str_array(value));
        }
    }

    pub fn encode_to_str_array<T>(list: &Vec<T>) -> Value
    where
        T: ToString,
    {
        let mut result = Vec::new();
        for item in list {
            let item = item.to_string();
            result.push(Value::String(item));
        }

        Value::Array(result)
    }

    pub fn decode_str_array_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Vec<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match obj.get(key) {
            Some(v) => Self::decode_from_str_array(v),
            None => Ok(vec![]),
        }
    }

    pub fn decode_option_str_array_field<T>(
        obj: &Map<String, Value>,
        key: &str,
    ) -> BuckyResult<Option<Vec<T>>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match obj.get(key) {
            Some(v) => Self::decode_from_str_array(v).map(|v| Some(v)),
            None => Ok(None),
        }
    }

    pub fn decode_from_str_array<T>(v: &Value) -> BuckyResult<Vec<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let list = v.as_array().ok_or_else(|| {
            let msg = format!("invalid json field, except array: {}", v);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let mut result = Vec::new();
        for item in list {
            let item = Self::decode_from_string(item)?;
            result.push(item);
        }

        Ok(result)
    }

    pub fn encode_as_list<T>(obj: &mut Map<String, Value>, key: &str, value: &Vec<T>)
    where
        T: JsonCodec<T>,
    {
        obj.insert(key.to_owned(), Self::encode_to_array(value));
    }

    pub fn encode_as_option_list<T>(obj: &mut Map<String, Value>, key: &str, value: Option<&Vec<T>>)
    where
        T: JsonCodec<T>,
    {
        if let Some(list) = value {
            obj.insert(key.to_owned(), Self::encode_to_array(list));
        }
    }

    pub fn encode_to_array<T>(list: &Vec<T>) -> Value
    where
        T: JsonCodec<T>,
    {
        let mut result = Vec::new();
        for item in list {
            let item = T::encode_value(item);
            result.push(item);
        }

        Value::Array(result)
    }

    pub fn decode_array_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Vec<T>>
    where
        T: JsonCodec<T>,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_from_array(v)
    }

    pub fn decode_option_array_field<T>(
        obj: &Map<String, Value>,
        key: &str,
    ) -> BuckyResult<Option<Vec<T>>>
    where
        T: JsonCodec<T>,
    {
        let ret = match obj.get(key) {
            Some(v) => Some(Self::decode_from_array(v)?),
            None => None,
        };

        Ok(ret)
    }

    pub fn decode_from_array<T>(v: &Value) -> BuckyResult<Vec<T>>
    where
        T: JsonCodec<T>,
    {
        let list = v.as_array().ok_or_else(|| {
            let msg = format!("invalid json field, except array: {}", v);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let mut result = Vec::new();
        for item in list {
            let item = T::decode_value(item)?;
            result.push(item);
        }

        Ok(result)
    }

    pub fn encode_field<T>(obj: &mut Map<String, Value>, key: impl ToString, value: &T)
    where
        T: JsonCodec<T>,
    {
        obj.insert(key.to_string(), value.encode_value());
    }

    pub fn encode_option_field<T>(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: Option<&T>,
    ) where
        T: JsonCodec<T>,
    {
        if let Some(value) = value {
            obj.insert(key.to_string(), value.encode_value());
        }
    }

    pub fn decode_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
    {
        let v = obj.get(key).ok_or_else(|| {
            let msg = format!("field not found: {}", key);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::NotFound, msg)
        })?;

        Self::decode_from_object(v)
    }

    pub fn decode_option_field<T>(obj: &Map<String, Value>, key: &str) -> BuckyResult<Option<T>>
    where
        T: JsonCodec<T>,
    {
        match obj.get(key) {
            Some(v) => {
                let obj = Self::decode_from_object(v)?;
                Ok(Some(obj))
            }
            None => Ok(None),
        }
    }

    pub fn decode_from_object<T>(v: &Value) -> BuckyResult<T>
    where
        T: JsonCodec<T>,
    {
        if !v.is_object() {
            let msg = format!("invalid object field: {:?}", v);
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        T::decode_json(v.as_object().unwrap())
    }
}

impl JsonCodec<BuckyError> for BuckyError {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        let code: u16 = self.code().into();
        obj.insert("code".to_owned(), Value::String(code.to_string()));

        obj.insert("msg".to_owned(), Value::String(self.msg().to_owned()));

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut code = BuckyErrorCode::Unknown;
        let mut msg: String = "".to_owned();

        for (k, v) in obj {
            match k.as_str() {
                "code" => {
                    // 支持number和string两种模式
                    let v: u16 = JsonCodecHelper::decode_to_int(v)?;
                    code = BuckyErrorCode::from(v);
                }

                "msg" => {
                    msg = v.as_str().unwrap_or("").to_owned();
                }

                u @ _ => {
                    warn!("unknown bucky error field: {}", u);
                }
            }
        }

        Ok(Self::new(code, msg))
    }
}

impl JsonCodec<NameLink> for NameLink {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        match self {
            Self::ObjectLink(id) => {
                obj.insert("t".to_owned(), Value::String("object".to_owned()));
                obj.insert("v".to_owned(), Value::String(id.to_string()));
            }
            Self::OtherNameLink(other) => {
                obj.insert("t".to_owned(), Value::String("name".to_owned()));
                obj.insert("v".to_owned(), Value::String(other.clone()));
            }
            Self::IPLink(addr) => {
                obj.insert("t".to_owned(), Value::String("ip".to_owned()));
                obj.insert("v".to_owned(), Value::String(addr.to_string()));
            }
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let t: String = JsonCodecHelper::decode_string_field(obj, "t")?;
        match t.as_ref() {
            "object" => Ok(NameLink::ObjectLink(JsonCodecHelper::decode_string_field(
                obj, "v",
            )?)),
            "name" => Ok(NameLink::OtherNameLink(
                JsonCodecHelper::decode_string_field(obj, "v")?,
            )),
            "ip" => Ok(NameLink::IPLink(JsonCodecHelper::decode_string_field(
                obj, "v",
            )?)),
            v @ _ => {
                let msg = format!("invalid name link type: {}", v);
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }
}

impl<T> JsonCodec<BuckyResult<T>> for BuckyResult<T>
where
    T: JsonCodec<T>,
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        match self {
            Ok(v) => JsonCodecHelper::encode_field(&mut obj, "value", v),
            Err(e) => JsonCodecHelper::encode_field(&mut obj, "error", e),
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        match JsonCodecHelper::decode_option_field(obj, "value")? {
            Some(v) => Ok(Ok(v)),
            None => match JsonCodecHelper::decode_option_field(obj, "error")? {
                Some(e) => Ok(Err(e)),
                None => {
                    let msg = format!("invalid BuckyResult format: {:?}", obj);
                    error!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                }
            },
        }
    }
}

impl JsonCodec<Self> for ObjectMapContentItem {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "content_type", &self.content_type());
        match &self {
            Self::Map((key, value)) => {
                JsonCodecHelper::encode_string_field(&mut obj, "key", key);
                JsonCodecHelper::encode_string_field(&mut obj, "value", value);
            }
            Self::DiffMap((key, value)) => {
                JsonCodecHelper::encode_string_field(&mut obj, "key", key);
                if let Some(value) = &value.prev {
                    JsonCodecHelper::encode_string_field(&mut obj, "prev", value);
                }
                if let Some(value) = &value.altered {
                    JsonCodecHelper::encode_string_field(&mut obj, "altered", value);
                }
                if let Some(value) = &value.diff {
                    JsonCodecHelper::encode_string_field(&mut obj, "diff", value);
                }
            }
            Self::Set(value) => {
                JsonCodecHelper::encode_string_field(&mut obj, "value", value);
            }
            Self::DiffSet(value) => {
                if let Some(value) = &value.prev {
                    JsonCodecHelper::encode_string_field(&mut obj, "prev", value);
                }
                if let Some(value) = &value.altered {
                    JsonCodecHelper::encode_string_field(&mut obj, "altered", value);
                }
            }
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let content_type: ObjectMapSimpleContentType =
            JsonCodecHelper::decode_string_field(obj, "content_type")?;
        let ret = match content_type {
            ObjectMapSimpleContentType::Map => {
                let key = JsonCodecHelper::decode_string_field(obj, "key")?;
                let value = JsonCodecHelper::decode_string_field(obj, "value")?;
                Self::Map((key, value))
            }
            ObjectMapSimpleContentType::DiffMap => {
                let key = JsonCodecHelper::decode_string_field(obj, "key")?;
                let prev = JsonCodecHelper::decode_option_string_field(obj, "prev")?;
                let altered = JsonCodecHelper::decode_option_string_field(obj, "altered")?;
                let diff = JsonCodecHelper::decode_option_string_field(obj, "diff")?;
                let item = ObjectMapDiffMapItem {
                    prev,
                    altered,
                    diff,
                };
                Self::DiffMap((key, item))
            }
            ObjectMapSimpleContentType::Set => {
                let value = JsonCodecHelper::decode_string_field(obj, "value")?;
                Self::Set(value)
            }
            ObjectMapSimpleContentType::DiffSet => {
                let prev = JsonCodecHelper::decode_option_string_field(obj, "prev")?;
                let altered = JsonCodecHelper::decode_option_string_field(obj, "altered")?;
                let item = ObjectMapDiffSetItem { prev, altered };
                Self::DiffSet(item)
            }
        };

        Ok(ret)
    }
}

impl JsonCodec<Self> for Vec<ObjectMapContentItem> {
    fn decode_value(value: &Value) -> BuckyResult<Self> {
        JsonCodecHelper::decode_from_array(value)
    }

    fn encode_value(&self) -> Value {
        JsonCodecHelper::encode_to_array(self)
    }
}
