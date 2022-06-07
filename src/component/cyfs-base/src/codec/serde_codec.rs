use crate::{NamedObjectId, ObjectType, ChunkId};

use serde::de::{self, Visitor, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::str::FromStr;


impl<T: ObjectType> Serialize for NamedObjectId<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

struct NamedObjectIdVisitor<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<'de, T: ObjectType> Visitor<'de> for NamedObjectIdVisitor<T> {
    type Value = NamedObjectId<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("base58 encoded string object id")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match NamedObjectId::from_str(v) {
            Ok(ret) => Ok(ret),
            Err(e) => {
                let msg = format!("invalid object id string: {}, {}", v, e);
                Err(E::custom(msg))
            }
        }
    }
}

impl<'de, T: ObjectType> Deserialize<'de> for NamedObjectId<T> {
    fn deserialize<D>(deserializer: D) -> Result<NamedObjectId<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(NamedObjectIdVisitor { _phantom: std::marker::PhantomData })
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

struct ChunkIdVisitor {
}

impl<'de> Visitor<'de> for ChunkIdVisitor {
    type Value = ChunkId;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("base58 encoded string chunk id")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match ChunkId::from_str(v) {
            Ok(ret) => Ok(ret),
            Err(e) => {
                let msg = format!("invalid chunk id string: {}, {}", v, e);
                Err(E::custom(msg))
            }
        }
    }
}

impl<'de> Deserialize<'de> for ChunkId {
    fn deserialize<D>(deserializer: D) -> Result<ChunkId, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ChunkIdVisitor { })
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::str::FromStr;
    use serde::*;

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
    }
}