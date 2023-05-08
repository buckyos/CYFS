
pub struct SnBenchPingException {
    wrong_public_key: u64,
    wrong_private_key: u64,
}

impl SnBenchPingException {
    pub fn default() -> Self {
        SnBenchPingException {
            wrong_public_key: 0,
            wrong_private_key: 0,
        }
    }
}

pub struct SnBenchCallException {
    wrong_public_key: u64,
    wrong_private_key: u64,
    remote_offline: u64,
    remote_not_exist: u64,
}
