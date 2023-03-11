// The current database version
pub(super) const CURRENT_VERSION: i32 = 0;
const SET_DB_VERSION: &'static str = concat!("PRAGMA USER_VERSION = ", 0);

pub(super) const OBJECT_RELATION_CACHE_INIT: &'static str = r#"
CREATE TABLE IF NOT EXISTS cache_object_relation (
    /* version 0 */
    object_id TEXT NOT NULL, 
    relation_type INTEGER,
    relation TEXT NOT NULL,
    target_object_id TEXT,

    insert_time INTEGER,
    last_access_time INTEGER,
    
    PRIMARY KEY(object_id, relation_type, relation)
);"#;

pub(super) const INIT_OBJECT_RELATION_CACHE_LIST: [&'static str; 1] = [
    OBJECT_RELATION_CACHE_INIT,
];