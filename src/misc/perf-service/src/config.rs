use std::fmt::{Display, Formatter};
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, TStringVisitor};
use cyfs_lib::SharedCyfsStack;
use crate::storage::StorageConfig;
use cyfs_perf_base::PERF_DEC_ID;

#[derive(PartialEq)]
pub enum StackType {
    OOD,
    Runtime,
    Other(u16, u16)
}

impl Default for StackType {
    fn default() -> Self {
        StackType::OOD
    }
}

impl Display for StackType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StackType::OOD => write!(f, "ood"),
            StackType::Runtime => write!(f, "runtime"),
            StackType::Other(http_port, ws_port) => write!(f, "{}:{}", http_port, ws_port)
        }
    }
}

impl FromStr for StackType {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s {
            "ood" => Ok(StackType::OOD),
            "runtime" => Ok(StackType::Runtime),
            v @ _ => {
                let ports: Vec<&str> = v.split(":").collect();
                if ports.len() != 2 {
                    let msg = format!("stack type str {} invalid. Must have two port numbers separated by a colon", v);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
                let http_port;
                let ws_port;
                match ports[0].parse::<u16>() {
                    Ok(port) => {http_port = port},
                    Err(_) => {
                        let msg = format!("stack http port {} invalid. Must u16 port number", ports[0]);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                };

                match ports[1].parse::<u16>() {
                    Ok(port) => {ws_port = port},
                    Err(_) => {
                        let msg = format!("stack ws port {} invalid. Must u16 port number", ports[1]);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                };

                Ok(StackType::Other(http_port, ws_port))
            }
        }
    }
}

impl<'de> Deserialize<'de> for StackType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TStringVisitor::<Self>::new())
    }
}

impl Serialize for StackType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PerfConfig {
    pub stack_type: StackType,
    pub storage: StorageConfig
}

impl Default for PerfConfig {
    fn default() -> Self {
        Self {
            stack_type: StackType::default(),
            storage: StorageConfig::default(),
        }
    }
}

pub(crate) async fn get_stack(config: StackType) -> BuckyResult<SharedCyfsStack> {
    let dec_id = Some(PERF_DEC_ID.clone());
    let stack = match config {
        StackType::OOD => {
            SharedCyfsStack::open_default(dec_id).await
        },
        StackType::Runtime => {
            SharedCyfsStack::open_runtime(dec_id).await
        },
        StackType::Other(http_port, ws_port) => {
            SharedCyfsStack::open_with_port(dec_id, http_port, ws_port).await
        }
    }?;

    stack.online().await?;

    Ok(stack)
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use crate::config::{PerfConfig, StackType};
    use crate::storage::mongo::MongoConfig;
    use crate::storage::{DatabaseConfig, StorageConfig};

    #[test]
    fn print_config() {
        let stack_type = StackType::from_str("2547:9885").unwrap();
        if let StackType::Other(http_port, ws_port) = stack_type {
            assert_eq!(http_port, 2547);
            assert_eq!(ws_port, 9885);
        } else {
            assert!(false);
        }

        println!("normal config: \n{}", toml::to_string(&PerfConfig {
            stack_type: StackType::OOD,
            storage: StorageConfig {
                isolate: Some("isolate".to_owned()),
                database: DatabaseConfig::MongoDB(MongoConfig { mongo_url: "mongodb://localhost:21731".to_string() })
            }
        }).unwrap());

        println!("runtime config: \n{}", toml::to_string(&PerfConfig {
            stack_type: StackType::Runtime,
            storage: StorageConfig {
                isolate: Some("isolate2".to_owned()),
                database: DatabaseConfig::MongoDB(MongoConfig { mongo_url: "mongodb://localhost:21731".to_string() })
            }
        }).unwrap());

        println!("normal config: \n{}", toml::to_string(&PerfConfig {
            stack_type: StackType::Other(21001, 21002),
            storage: StorageConfig {
                isolate: None,
                database: DatabaseConfig::MongoDB(MongoConfig { mongo_url: "mongodb://localhost:21731".to_string() })
            }
        }).unwrap());

        let config_str = r#"
stack_type = "2132:8876"

[storage.database.mongodb]
mongo_url = "mongodb://localhost:21731"
"#;
        let config: PerfConfig  = toml::from_str(config_str).unwrap();
        assert!(config.stack_type == StackType::Other(2132, 8876));
        assert!(config.storage.isolate.is_none());
        if let DatabaseConfig::MongoDB(config) = config.storage.database {
            assert_eq!(config.mongo_url.as_str(), "mongodb://localhost:21731");
        } else {
            assert!(false);
        }
    }
}
