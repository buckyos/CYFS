use cyfs_base::*;
use cyfs_lib::*;


use std::fmt;

pub(super) struct RouterHandlerRequestRouterInfo {
    // 来源设备
    pub source: DeviceId,

    // 最终target和方向
    pub target: Option<DeviceId>,
    pub direction: Option<ZoneDirection>,

    // 下一条设备和方向
    pub next_hop: Option<DeviceId>,
    pub next_direction: Option<ZoneDirection>,
}

impl RouterHandlerRequestRouterInfo {
    pub fn clear(&mut self) {
        self.target = None;
        self.direction = None;
        self.next_hop = None;
        self.next_direction = None;
    }
}

impl fmt::Display for RouterHandlerRequestRouterInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source: {}", self.source.to_string())?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }
        if let Some(direction) = &self.direction {
            write!(f, ", direction: {}", direction.to_string())?;
        }
        if let Some(next_hop) = &self.next_hop {
            write!(f, ", next_hop: {}", next_hop.to_string())?;
        }
        if let Some(next_direction) = &self.next_direction {
            write!(f, ", next_direction: {}", next_direction.to_string())?;
        }

        Ok(())
    }
}

/*
impl JsonCodec<RouterHandlerRequestRouterInfo> for RouterHandlerRequestRouterInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "source", &self.source);

        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "direction", self.direction.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "next_hop", self.next_hop.as_ref());
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "next_direction",
            self.next_direction.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<RouterHandlerRequestRouterInfo> {
        Ok(Self {
            source: JsonCodecHelper::decode_string_field(obj, "source")?,
            direction: JsonCodecHelper::decode_option_string_field(obj, "direction")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            next_hop: JsonCodecHelper::decode_option_string_field(obj, "next_hop")?,
            next_direction: JsonCodecHelper::decode_option_string_field(obj, "next_direction")?,
        })
    }
}
*/