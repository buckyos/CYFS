use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use std::str::FromStr;
use std::convert::TryFrom;


pub struct TomlHelper;
impl TomlHelper {
    pub fn decode_from_string<T>(v: &toml::Value) -> BuckyResult<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        if !v.is_str() {
            let msg = format!("invalid toml field, except string: {}", v);
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

    pub fn decode_from_boolean(v: &toml::Value) -> BuckyResult<bool> {
        let v = v.as_bool().ok_or_else(|| {
            let msg = format!("invalid toml field, except bool: {}", v);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(v)
    }

    pub fn decode_string_field<T>(obj: &toml::value::Table, key: &str) -> BuckyResult<T>
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
        obj: &toml::value::Table,
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

    pub fn decode_to_int<T>(v: &toml::Value) -> BuckyResult<T>
    where
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
        <T as FromStr>::Err: std::fmt::Display,
        <T as TryFrom<u64>>::Error: std::fmt::Display,
        <T as TryFrom<i64>>::Error: std::fmt::Display,
    {
        if v.is_str() {
            let v = T::from_str(v.as_str().unwrap()).map_err(|e| {
                let msg = format!(
                    "parse toml string to int error: value={}, {}",
                    v.as_str().unwrap(),
                    e
                );
                warn!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            Ok(v)
        } else if v.is_integer() {
            if v.is_integer() {
                let v = T::try_from(v.as_integer().unwrap()).map_err(|e| {
                    let msg = format!(
                        "parse toml integer to int error: value={}, {}",
                        v.as_integer().unwrap(),
                        e
                    );
                    warn!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;
                Ok(v)
            } else {
                let msg = format!(
                    "parse toml integer to int error: value={}",
                    v.as_integer().unwrap(),
                );
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        } else {
            let msg = format!("invalid toml field, except string or integer: {}", v);
            warn!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        }
    }


    pub fn extract_sub_node(mut root: toml::Value, path: &str) -> BuckyResult<toml::Value> {
        let parts: Vec<&str> = path.split('.').collect();

        for part in parts {
            root = Self::extract_node(root, part)?;
        }

        Ok(root)
    }

    pub fn extract_node(root: toml::Value, name: &str) -> BuckyResult<toml::Value> {
        match root {
            toml::Value::Table(mut cfg) => match cfg.remove(name) {
                Some(v) => Ok(v),
                None => {
                    let msg = format!("sub node not found! name={}", name);
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
                }
            }

            _ => {
                let msg = format!(
                    "node is not table! config={}", toml::to_string(&root).unwrap()
                );
                error!("{}", msg);
                Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
            }
        }
    }
}