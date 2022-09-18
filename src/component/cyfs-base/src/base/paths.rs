
// Every app has one
pub const CYFS_GLOBAL_STATE_META_PATH: &str = "/.cyfs/meta";

// Friends, in system dec's global state
pub const CYFS_FRIENDS_PATH: &str = "/user/friends";
pub const CYFS_FRIENDS_LIST_PATH: &str = "/user/friends/list";
pub const CYFS_FRIENDS_OPTION_PATH: &str = "/user/friends/option";

// AppManager related paths
pub const CYFS_APP_LOCAL_LIST_PATH: &str = "/app/manager/local_list";
pub const CYFS_APP_LOCAL_STATUS_PATH: &str = "/app/${DecAppId}/local_status";

// Known zones in local-cache
pub const CYFS_KNOWN_ZONES_PATH: &str = "/data/known-zones";

// Virtual path for handler and api 
pub const CYFS_API_VIRTUAL_PATH: &str = "/.cyfs/api";
pub const CYFS_HANDLER_VIRTUAL_PATH: &str = "/.cyfs/api/handler";
pub const CYFS_CRYPTO_VIRTUAL_PATH: &str = "/.cyfs/api/crypto";