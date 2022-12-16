use cyfs_base::*;
use cyfs_lib::*;

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;

#[repr(u8)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum NameItemStatus {
    Init = 0,
    Ready = 1,
    NotFound = 2,
    Error = 3,
}

impl Into<u8> for NameItemStatus {
    fn into(self) -> u8 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

impl TryFrom<u8> for NameItemStatus {
    type Error = BuckyError;

    fn try_from(status: u8) -> BuckyResult<Self> {
        let ret = match status {
            0 => Self::Init,
            1 => Self::Ready,
            2 => Self::NotFound,
            3 => Self::Error,
            v @ _ => {
                error!("unknown NameItemStatus value: {}", v);
                return Err(BuckyError::from(BuckyErrorCode::NotSupport));
            }
        };

        Ok(ret)
    }
}

impl fmt::Display for NameItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::Init => "init",
            Self::Ready => "ready",
            Self::Error => "error",
            Self::NotFound => "not_found",
        };

        fmt::Display::fmt(s, f)
    }
}

pub(super) struct NameCacheItem {
    pub status: NameItemStatus,

    // 最后一次从meta查找的结果，只有成功后才更新status和link
    pub last_resolve_status: NameItemStatus,

    // 最后一次从meta查找时刻
    pub last_tick: u64,

    pub link: Option<NameLink>,
}

impl fmt::Display for NameCacheItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({},{},{:?})", self.status, self.last_tick, self.link)
    }
}

impl NameCacheItem {
    pub fn new() -> Self {
        Self {
            status: NameItemStatus::Init,
            last_resolve_status: NameItemStatus::Init,
            last_tick: bucky_time_now(),
            link: None,
        }
    }

    pub fn reset(&mut self, name: &str) {
        info!(
            "will reset name cache item status: name={}, status={}",
            name, self.status
        );

        self.status = NameItemStatus::Init;
        self.last_resolve_status = NameItemStatus::Init;
        self.last_tick = bucky_time_now();
        self.link = None;
    }
}

pub(super) struct NameCache {
    all: HashMap<String, NameCacheItem>,
}

impl Default for NameCache {
    fn default() -> Self {
        Self::new()
    }
}

impl NameCache {
    pub fn new() -> Self {
        Self {
            all: HashMap::new(),
        }
    }

    pub fn get(&mut self, name: &str) -> &mut NameCacheItem {
        let entry = self
            .all
            .entry(name.to_owned())
            .or_insert_with(|| NameCacheItem::new());
        entry
    }

    pub fn try_get(&mut self, name: &str) -> Option<&mut NameCacheItem> {
        self.all.get_mut(name)
    }
    
    pub fn load() {}

    fn save() {}
}

impl JsonCodec<NameCacheItem> for NameCacheItem {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        let status: u8 = self.status.clone().into();
        obj.insert("status".to_owned(), Value::String(status.to_string()));
        obj.insert(
            "last_tick".to_owned(),
            Value::String(self.last_tick.to_string()),
        );

        if let Some(ref link) = self.link {
            obj.insert("link".to_owned(), Value::Object(link.encode_json()));
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut status = None;
        let mut last_tick = None;
        let mut link = None;

        for (k, v) in obj {
            match k.as_str() {
                "status" => {
                    let v: u8 = JsonCodecHelper::decode_from_string(v)?;

                    status = Some(NameItemStatus::try_from(v)?);
                }
                "last_tick" => {
                    last_tick = Some(JsonCodecHelper::decode_from_string(v)?);
                }
                "link" => {
                    link = Some(JsonCodecHelper::decode_from_object(v)?);
                }
                v @ _ => {
                    warn!("unknown name cache item field: {}", v);
                }
            }
        }

        if status.is_none() || last_tick.is_none() {
            let msg = format!("name cache item field missing: status/last_tick");
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        // 这里需要确保状态和值匹配
        let mut status = status.unwrap();
        if status == NameItemStatus::Ready {
            if link.is_none() {
                error!("unmatch name cache item status and link! status={}", status);
                status = NameItemStatus::Init;
            }
        } else {
            if link.is_some() {
                error!("unmatch name cache item status and link! status={}", status);
                link = None;
            }
        }

        let ret = Self {
            status: status.clone(),
            last_resolve_status: status,
            last_tick: last_tick.unwrap(),
            link,
        };

        Ok(ret)
    }
}

impl JsonCodec<NameCache> for NameCache {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        for (name, item) in self.all.iter() {
            if item.status == NameItemStatus::Ready || item.status == NameItemStatus::Error {
                obj.insert(name.clone(), Value::Object(item.encode_json()));
            }
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut ret = Self::new();

        for (k, v) in obj {
            match JsonCodecHelper::decode_from_object(v) {
                Ok(item) => {
                    info!("load name cache item success: {}", item);
                    ret.all.insert(k.to_owned(), item);
                }
                Err(e) => {
                    error!("load name cache item failed! name={}, {}", k, e);
                }
            }
        }
        Ok(ret)
    }
}

declare_collection_codec_for_json_codec!(NameCache);
