use std::time::Duration;

pub const GROUP_METHOD_UPDATE: &str = ".update";
pub const GROUP_METHOD_DECIDE: &str = ".decide";
pub enum GroupUpdateDecide {
    Accept = 1,
    Reject = 2,
    JoinAdmin = 3,
    JoinMember = 4,
}

// Some config
pub const NETWORK_TIMEOUT: Duration = Duration::from_millis(5000);
pub const HOTSTUFF_TIMEOUT_DEFAULT: u64 = 5000;
pub const CHANNEL_CAPACITY: usize = 1000;
pub const TIME_PRECISION: Duration = Duration::from_millis(60000);
pub const SYNCHRONIZER_TIMEOUT: u64 = 500;
pub const SYNCHRONIZER_TRY_TIMES: usize = 3;
