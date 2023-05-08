
pub struct SnBenchPingException {
    wrong_public_key: bool,
    wrong_private_key: bool,
}

impl SnBenchPingException {
    pub fn default() -> Self {
        SnBenchPingException {
            wrong_public_key: false,
            wrong_private_key: false,
        }
    }
}

pub struct SnBenchCallException {
    wrong_public_key: bool,
    wrong_private_key: bool,
    remote_offline: bool,
    remote_not_exist: bool,
}

impl SnBenchCallException {
    pub fn default() -> Self {
        SnBenchCallException {
            wrong_public_key: false,
            wrong_private_key: false,
            remote_offline: false,
            remote_not_exist: false,
        }
    }
}