use cyfs_base::*;

use async_trait::async_trait;
use int_enum::IntEnum;
use std::convert::TryFrom;
use std::str::FromStr;

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, IntEnum)]
pub enum TrackerDirection {
    Unknown = 0,
    From = 1,
    To = 2,
    Store = 3,
}

impl Into<u8> for TrackerDirection {
    fn into(self) -> u8 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

impl From<u8> for TrackerDirection {
    fn from(code: u8) -> Self {
        match TrackerDirection::from_int(code) {
            Ok(code) => code,
            Err(e) => {
                error!("unknown TrackerDirection code: {} {}", code, e);
                TrackerDirection::Unknown
            }
        }
    }
}

// path: [range_begin, range_end)
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PostionFileRange {
    pub path: String,
    pub range_begin: u64,
    pub range_end: u64,
}

impl PostionFileRange {
    pub fn encode(&self) -> String {
        format!("{}:{}:{}", self.range_begin, self.range_end, self.path)
    }

    pub fn decode(value: &str) -> BuckyResult<Self> {
        let parts: Vec<&str> = value.split(':').collect();
        let range_begin = u64::from_str(parts[0]).map_err(|e| {
            let msg = format!("invalid range_begin string: {}, {}", parts[0], e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let range_end = u64::from_str(parts[1]).map_err(|e| {
            let msg = format!("invalid range_end string: {}, {}", parts[0], e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let path = parts[2..].join(":");
        Ok(Self {
            path,
            range_begin,
            range_end,
        })
    }
}

impl ToString for PostionFileRange {
    fn to_string(&self) -> String {
        self.encode()
    }
}

impl FromStr for PostionFileRange {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        PostionFileRange::decode(value)
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum TrackerPostion {
    Unknown(String),
    Device(DeviceId),
    File(String),
    FileRange(PostionFileRange),
    ChunkManager,
}

impl Into<(u8, String)> for TrackerPostion {
    fn into(self) -> (u8, String) {
        match self {
            Self::Unknown(v) => (0, v),
            Self::Device(device_id) => (1, device_id.to_string()),
            Self::File(v) => (2, v),
            Self::FileRange(v) => (3, v.to_string()),
            TrackerPostion::ChunkManager => (4, "ChunkManager".to_string())
        }
    }
}

impl TryFrom<(u8, String)> for TrackerPostion {
    type Error = BuckyError;

    fn try_from((code, value): (u8, String)) -> Result<Self, Self::Error> {
        let ret = match code {
            0 => Self::Unknown(value),
            1 => {
                let device_id = DeviceId::from_str(&value).map_err(|e| {
                    let msg = format!("invalid device_id string: {}, {}", value, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Self::Device(device_id)
            }
            2 => Self::File(value),
            3 => {
                let file_range = PostionFileRange::from_str(&value)?;
                Self::FileRange(file_range)
            }
            4 => {
                Self::ChunkManager
            }
            _ => {
                error!("unknown TrackerPostion code: {}", code);
                Self::Unknown(value)
            }
        };

        Ok(ret)
    }
}

pub struct AddTrackerPositonRequest {
    pub id: String,
    pub direction: TrackerDirection,
    pub pos: TrackerPostion,
    pub flags: u32,
}

pub struct RemoveTrackerPositionRequest {
    pub id: String,
    pub direction: Option<TrackerDirection>,
    pub pos: Option<TrackerPostion>,
}

pub struct GetTrackerPositionRequest {
    pub id: String,
    pub direction: Option<TrackerDirection>,
}

#[derive(Debug)]
pub struct TrackerPositionCacheData {
    pub direction: TrackerDirection,
    pub pos: TrackerPostion,
    pub insert_time: u64,
    pub flags: u32,
}

#[async_trait]
pub trait TrackerCache: Sync + Send + 'static {
    fn clone(&self) -> Box<dyn TrackerCache>;

    async fn add_position(&self, req: &AddTrackerPositonRequest) -> BuckyResult<()>;
    async fn remove_position(&self, req: &RemoveTrackerPositionRequest) -> BuckyResult<usize>;

    async fn get_position(
        &self,
        req: &GetTrackerPositionRequest,
    ) -> BuckyResult<Vec<TrackerPositionCacheData>>;
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::convert::TryFrom;
    use std::str::FromStr;

    #[test]
    fn test_file_range() {
        let item = PostionFileRange {
            path: "xxxxxx:xxxx".to_owned(),
            range_begin: 1000,
            range_end: 2000,
        };

        let value = item.to_string();
        println!("{}", value);

        let r_item = PostionFileRange::from_str(&value).unwrap();
        assert!(r_item.path == item.path);
        assert!(r_item.range_begin == item.range_begin);
        assert!(r_item.range_end == item.range_end);

        let r_item2 = r_item.clone();
        let pos = TrackerPostion::FileRange(r_item);
        let value: (u8, String) = pos.into();
        let r_pos = TrackerPostion::try_from(value).unwrap();
        if let TrackerPostion::FileRange(v) = r_pos {
            assert!(v == r_item2);
        } else {
            assert!(false);
        }
    }
}
