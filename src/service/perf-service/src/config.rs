use std::collections::HashMap;
use std::path::Path;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode};
use serde_json::Value;

pub struct ConfigManager {
    keys: HashMap<String, String>
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new()
        }
    }

    pub fn init<P: AsRef<Path>>(&mut self, config: P) -> BuckyResult<()>{
        let file = std::fs::File::open(config)?;
        let root: Value = serde_json::from_reader(&file)?;
        for (k, v) in root.as_object().ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))? {
            self.keys.insert(k.clone(), v.as_str().ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?.to_owned());
        }

        Ok(())
    }

    pub fn get(&self, dec: &str) -> BuckyResult<String> {
        self.keys.get(dec).map(|v|v.clone()).ok_or(BuckyError::from(BuckyErrorCode::NotFound))
    }
}