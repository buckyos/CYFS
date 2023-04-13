use cyfs_base::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SNMode {
    Normal,
    None,
}

impl SNMode {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Normal => "normal",
            Self::None => "none",
        }
    }
}

impl Default for SNMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for SNMode {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SNMode {
    type Err = BuckyError;

    fn from_str(str: &str) -> BuckyResult<Self> {
        let ret = match str {
            "normal" => Self::Normal,
            "none" => Self::None,
            _ => {
                let msg = format!("unknown SNMode {}", str);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone)]
pub struct BdtStackParams {
    pub device: Device,
    pub tcp_port_mapping: Vec<(Endpoint, u16)>,
    pub secret: PrivateKey,
    pub known_sn: Vec<Device>,
    pub known_device: Vec<Device>,
    pub known_passive_pn: Vec<Device>,
    pub udp_sn_only: Option<bool>,
    pub sn_mode: SNMode,

    // sn ping interval in seconds, default is 25s
    pub ping_interval: Option<u32>,
}
