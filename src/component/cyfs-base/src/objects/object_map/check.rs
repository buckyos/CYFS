use crate::*;

pub struct ObjectMapChecker;

impl ObjectMapChecker {
    pub fn check_key_value(key: &str) -> BuckyResult<()> {
        if key.len() == 0 {
            let msg = format!("empty objectmap key is invalid!");
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        if key.len() > OBJECT_MAP_KEY_MAX_LEN {
            let msg = format!(
                "objectmap key extend limit: key={}, len={}, maxlen={}",
                key,
                key.len(),
                OBJECT_MAP_KEY_MAX_LEN
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
        }

        if key.find('/').is_some() {
            let msg = format!("objectmap key cannot contain '/': {}", key);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(())
    }
}