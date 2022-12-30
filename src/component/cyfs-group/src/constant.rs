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
pub const ASYNC_TIMEOUT: Duration = Duration::from_millis(500);
pub const NETWORK_TIMEOUT: Duration = Duration::from_millis(5000);
pub const HOTSTUFF_TIMEOUT_DEFAULT: u64 = 5000;
pub const CHANNEL_CAPACITY: usize = 1000;
