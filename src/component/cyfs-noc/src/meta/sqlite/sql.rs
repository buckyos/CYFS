// 当前的数据库版本
pub(super) const CURRENT_VERSION: i32 = 0;

pub(super) const DATA_NAMEDOBJECT_META: &'static str = r#"
CREATE TABLE IF NOT EXISTS data_namedobject_meta (
    object_id TEXT PRIMARY KEY NOT NULL UNIQUE, 
    
    owner_id VARCHAR,
    create_dec_id VARCHAR,

    insert_time UNSIGNED BIG INT,
    update_time UNSIGNED BIG INT,
    
    object_update_time UNSIGNED BIG INT,
    object_expired_time UNSIGNED BIG INT,

    storage_category SMALLINT,

    context: TEXT,

    last_access_time UNSIGNED BIG INT,
    last_access_rpath TEXT,

    access: INTEGER,
)"#;

pub(super) const DATA_NAMEDOBJECT_META_INSERT_TIME_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `data_namedobject_meta_insert_time_index` on `data_namedobject_meta` (`insert_time`);
"#;

pub(super) const DATA_NAMEDOBJECT_META_INSERT_LAST_ACCESS_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `data_namedobject_meta_last_access_time_index` on `data_namedobject_meta` (`last_access_time`);
"#;

pub(super) const INIT_NAMEDOBJECT_META_SQL_LIST: [&'static str; 3] = [
    DATA_NAMEDOBJECT_META,
    DATA_NAMEDOBJECT_META_INSERT_TIME_INDEX,
    DATA_NAMEDOBJECT_META_INSERT_LAST_ACCESS_INDEX,
];


// For all version upgrades, MAIN_TABLE_UPDATE_LIST[CURRENT_VERSION - 1] is the corresponding upgrade sql
pub(super) const MAIN_TABLE_UPDATE_LIST: [[&'static str; 0]; CURRENT_VERSION as usize] = [];
