use cyfs_base::{BuckyError, BuckyErrorCode};
use regex::{Regex, RegexBuilder};
use wildmatch::WildMatch;

// 参考 https://nginx.org/en/docs/http/server_names.html

#[derive(Debug)]
pub(super) enum ServerName {
    All,
    None,
    Exact(String),
    WildCard(WildMatch),
    Regex(Regex),
}

impl ServerName {
    fn is_alphanumeric(byte: u8) -> bool {
        (byte >= b'a' && byte <= b'z')
            || (byte >= b'A' && byte <= b'Z')
            || (byte >= b'0' && byte <= b'9')
            || byte == b'-'
    }

    fn is_valid_host(hostname: &str) -> bool {
        !(hostname
            .bytes()
            .any(|byte| hostname.is_empty() || !Self::is_alphanumeric(byte))
            || hostname.ends_with('-')
            || hostname.starts_with('-'))
    }

    fn is_exact_host(host: &str) -> bool {
        host.split('.')
            .all(|hostname| Self::is_valid_host(hostname))
    }

    pub fn parse(value: &str) -> Result<ServerName, BuckyError> {
        let ret = if value.is_empty() {
            ServerName::None
        } else if value == "_" {
            ServerName::All
        } else if value.starts_with("*.") || value.ends_with(".*") {
            ServerName::WildCard(WildMatch::new(value))
        } else if value.starts_with(".") {
            // .example.org 等价于 *.example.org
            let value = format!("*{}", value);
            ServerName::WildCard(WildMatch::new(&value))
        } else if value.ends_with(".") {
            // www.example. 等价于 www.example.*
            let value = format!("{}*", value);
            ServerName::WildCard(WildMatch::new(&value))
        } else if Self::is_exact_host(value) {
            ServerName::Exact(value.to_lowercase())
        } else {
            match RegexBuilder::new(value).case_insensitive(true).build() {
                Ok(m) => ServerName::Regex(m),
                Err(e) => {
                    error!("invalid server name regex: {}, err={}", value, e);
                    return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
                }
            }
        };

        Ok(ret)
    }

    // TODO 多个wildcard匹配，增加基于长度的优先级，参考nginx的实现
    pub fn is_match(&self, host: Option<&str>) -> bool {
        if host.is_none() {
            return match self {
                ServerName::None => true,
                _ => false,
            };
        }

        let host = host.unwrap().to_lowercase();

        match self {
            ServerName::All => true,
            ServerName::Exact(m) => m.eq(&host),
            ServerName::WildCard(w) => w.matches(&host),
            ServerName::Regex(r) => r.is_match(&host),
            ServerName::None => unreachable!(),
        }
    }
}
