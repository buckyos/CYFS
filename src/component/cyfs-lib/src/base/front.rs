use cyfs_base::*;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum BrowserSanboxMode {
    Forbidden,
    Strict,
    Relaxed,
    None,
}

impl Default for BrowserSanboxMode {
    fn default() -> Self {
        Self::Strict
    }
}

impl BrowserSanboxMode {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Forbidden => "forbidden",
            Self::Strict => "strict",
            Self::Relaxed => "relaxed",
            Self::None => "none",
        }
    }
}

impl std::str::FromStr for BrowserSanboxMode {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s {
            "forbidden" => Self::Forbidden,
            "strict" => Self::Strict,
            "relaxed" => Self::Relaxed,
            "none" => Self::None,
            _ => {
                let msg = format!("unknown browser mode: {}", s);
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };
        Ok(mode)
    }
}
