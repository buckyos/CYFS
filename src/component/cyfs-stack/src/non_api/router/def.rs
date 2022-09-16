use cyfs_base::*;
use cyfs_lib::*;


use std::fmt;

pub(super) struct RouterHandlerRequestRouterInfo {
    // 来源设备
    pub source: RequestSourceInfo,

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
        write!(f, "{}", self.source)?;

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