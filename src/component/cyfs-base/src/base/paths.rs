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

pub const CYFS_GLOBAL_STATE_VIRTUAL_PATH: &str = "/.cyfs/api/global_state";
pub const CYFS_GLOBAL_STATE_ROOT_VIRTUAL_PATH: &str = "/.cyfs/api/global_state/root";

// System control cmds
pub const CYFS_SYSTEM_VIRTUAL_PATH: &str = "/.cyfs/api/system";
pub const CYFS_SYSTEM_ADMIN_VIRTUAL_PATH: &str = "/.cyfs/api/system/admin";
pub const CYFS_SYSTEM_ROLE_VIRTUAL_PATH: &str = "/.cyfs/api/system/role";
pub const CYFS_SYSTEM_APP_VIRTUAL_PATH: &str = "/.cyfs/api/system/app";

//App control cmds (e.g.: Start, Stop, Install, Uninstall)
pub const CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH: &str = "/.cyfs/api/system/app/cmd";
