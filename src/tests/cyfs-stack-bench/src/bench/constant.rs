pub const NON_ALL_IN_ONE: &str = "non-all-in-one";
pub const NON_PUT_OBJECT: &str = "non-inner-zone-diff-dec-put-object";
pub const NON_GET_OBJECT: &str = "non-inner-zone-diff-dec-get-object";
pub const NON_POST_OBJECT: &str = "non-inner-zone-diff-dec-post-object";
pub const NON_DELETE_OBJECT: &str = "non-inner-zone-diff-dec-delete-object";

pub const NON_OUTER_GET_OBJECT: &str = "non-outer-zone-diff-dec-post-object";
pub const NON_OUTER_POST_OBJECT: &str = "non-outer-zone-diff-dec-delete-object";

pub const GLOABL_STATE_ALL_IN_ONE: &str = "global-state-all-in-one";
pub const ROOT_STATE_MAP:&str = "root-state-inner-zone-map-all-in-one";
pub const ROOT_STATE_SET:&str = "root-state-inner-zone-set-all-in-one";
pub const LOCAL_CACHE_MAP: &str = "local-cache-map-all-in-one";
pub const LOCAL_CACHE_SET: &str = "local-cache-set-all-in-one";

pub const ROOT_STATE_CREATE_NEW_OPERATION: &str = "root-state-inner-zone-create-new-operation";
pub const ROOT_STATE_GET_OPERATION: &str = "root-state-inner-zone-get-operation";
pub const ROOT_STATE_REMOVE_OPERATION: &str = "root-state-inner-zone-remove-operation";
pub const ROOT_STATE_INSERT_OPERATION: &str = "root-state-inner-zone-insert-operation";
pub const ROOT_STATE_COMMIT_OPERATION: &str = "root-state-inner-zone-commit-operation";

pub const RMETA_INNER_ZONE_ACCESS: &str = "rmeta-inner-zone-access";

pub const STAT_METRICS_LIST: [&'static str; 18] = [
    NON_ALL_IN_ONE,

    NON_PUT_OBJECT,
    NON_GET_OBJECT,
    NON_POST_OBJECT,
    NON_DELETE_OBJECT,

    NON_OUTER_GET_OBJECT,
    NON_OUTER_POST_OBJECT,

    GLOABL_STATE_ALL_IN_ONE,
    LOCAL_CACHE_MAP,
    LOCAL_CACHE_SET,
    ROOT_STATE_MAP,
    ROOT_STATE_SET,
    ROOT_STATE_CREATE_NEW_OPERATION,
    ROOT_STATE_GET_OPERATION,
    ROOT_STATE_REMOVE_OPERATION,
    ROOT_STATE_INSERT_OPERATION,
    ROOT_STATE_COMMIT_OPERATION,

    RMETA_INNER_ZONE_ACCESS,

];
