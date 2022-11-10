pub const NON_ALL_IN_ONE: &str = "non-all-in-one";
pub const NON_PUT_OBJECT: &str = "non-inner-zone-diff-dec-put-object";
pub const NON_GET_OBJECT: &str = "non-inner-zone-diff-dec-get-object";
pub const NON_POST_OBJECT: &str = "non-inner-zone-diff-dec-post-object";
pub const NON_DELETE_OBJECT: &str = "non-inner-zone-diff-dec-delete-object";


pub const ROOT_STATE_ALL_IN_ONE: &str = "root-state-all-in-one";
pub const PATH_OP_ENV_CROSS_DEC:&str = "path-op-cross-dec";
pub const SINGLE_OP_ENV_CROSS_DEC: &str = "single_op_env-cross-dec";

pub const ROOT_STATE_GET_OPERATION: &str = "root-state-get-operation";
pub const ROOT_STATE_REMOVE_OPERATION: &str = "root-state-remove-operation";
pub const ROOT_STATE_INSERT_OPERATION: &str = "root-state-insert-operation";
pub const ROOT_STATE_COMMIT_OPERATION: &str = "root-state-commit-operation";

pub const STAT_METRICS_LIST: [&'static str; 5] = [
    NON_ALL_IN_ONE,
    NON_PUT_OBJECT,

    NON_GET_OBJECT,

    NON_POST_OBJECT,
    NON_DELETE_OBJECT,
];
