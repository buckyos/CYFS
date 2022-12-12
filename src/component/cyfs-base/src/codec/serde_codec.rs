use crate::{ChunkId, HashValue, NamedObjectId, ObjectType};

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use std::str::FromStr;
use std::convert::TryFrom;

// T with impl FromStr
pub struct TStringVisitor<T>
where
    T: FromStr,
{
    dummy: std::marker::PhantomData<T>,
}

impl<T> TStringVisitor<T>
where
    T: FromStr,
{
    pub fn new() -> Self {
        Self {
            dummy: std::marker::PhantomData,
        }
    }
}
impl<'de, T> Visitor<'de> for TStringVisitor<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("encoded string value error")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match T::from_str(v) {
            Ok(ret) => Ok(ret),
            Err(e) => {
                let msg = format!("invalid string value: {}, {}", v, e);
                Err(E::custom(msg))
            }
        }
    }
}

// T with impl TryFrom
pub struct TU8Visitor<T>
where
    T: TryFrom<u8>,
{
    dummy: std::marker::PhantomData<T>,
}

impl<T> TU8Visitor<T>
where
    T: TryFrom<u8>,
{
    pub fn new() -> Self {
        Self {
            dummy: std::marker::PhantomData,
        }
    }
}
impl<'de, T> Visitor<'de> for TU8Visitor<T>
where
    T: TryFrom<u8>,
    <T as TryFrom<u8>>::Error: std::fmt::Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("encoded u8 value error")
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match T::try_from(v) {
            Ok(ret) => Ok(ret),
            Err(e) => {
                let msg = format!("invalid u8 value: {}, {}", v, e);
                Err(E::custom(msg))
            }
        }
    }
}


// NamedObjectId
impl<T: ObjectType> Serialize for NamedObjectId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de, T: ObjectType> Deserialize<'de> for NamedObjectId<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}

// chunk_id
impl Serialize for ChunkId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ChunkId {
    fn deserialize<D>(deserializer: D) -> Result<ChunkId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<ChunkId>::new())
    }
}

// HashValue
impl Serialize for HashValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for HashValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}


#[cfg(test)]
mod test {
    use crate::*;
    use serde::*;
    use std::str::FromStr;

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
    struct OODStatus {
        pub device_id: DeviceId,
        pub people_id: PeopleId,
    }

    #[test]
    fn test_codec() {
        let people = "5r4MYfFAAwQGDHccsyTX1wnshnEu6UYW3SZ3AHnNm2g9";
        let ood = "5aSixgLkHa2NR4vSKJLYLPo5Av6CY3RJeFJegtF5iR1g";

        let status = OODStatus {
            people_id: PeopleId::from_str(people).unwrap(),
            device_id: DeviceId::from_str(ood).unwrap(),
        };

        let s = serde_json::to_string(&status).unwrap();
        println!("{}", s);

        let status2: OODStatus = serde_json::from_str(&s).unwrap();
        assert_eq!(status, status2);

        let hash = hash_data("test".as_bytes());
        let s = serde_json::to_string(&hash).unwrap();
        println!("{}", s);

        let hash2: HashValue = serde_json::from_str(&s).unwrap();
        assert_eq!(hash, hash2);
    }
}
