// 当前的数据库版本
pub(super) const CURRENT_VERSION: i32 = 1;

pub(super) const DATA_NAMEDOBJECT_META_INIT: &'static str = r#"
CREATE TABLE IF NOT EXISTS data_namedobject_meta (
    /* version 0 */
    object_id TEXT PRIMARY KEY NOT NULL UNIQUE, 
    
    owner_id TEXT,
    create_dec_id TEXT,

    insert_time INTEGER,
    update_time INTEGER,
    
    object_update_time INTEGER,
    object_expired_time INTEGER,

    storage_category SMALLINT,

    context TEXT,

    last_access_time INTEGER,
    last_access_rpath TEXT,

    access INTEGER,

    /* version 1 */
    object_type SMALLINT,
    object_create_time INTEGER,
    author BLOB,
    dec_id BLOB,
    prev BLOB,
    body_prev_version BLOB,
    ref_objs BLOB,

    nonce BLOB,
    difficulty INTEGER
);"#;

pub(super) const DATA_NAMEDOBJECT_META_INSERT_TIME_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `data_namedobject_meta_insert_time_index` on `data_namedobject_meta` (`insert_time`);
"#;

pub(super) const DATA_NAMEDOBJECT_META_INSERT_LAST_ACCESS_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `data_namedobject_meta_last_access_time_index` on `data_namedobject_meta` (`last_access_time`);
"#;

pub(super) const INIT_NAMEDOBJECT_META_SQL_LIST: [&'static str; 3] = [
    DATA_NAMEDOBJECT_META_INIT,
    DATA_NAMEDOBJECT_META_INSERT_TIME_INDEX,
    DATA_NAMEDOBJECT_META_INSERT_LAST_ACCESS_INDEX,
];

// version 1 alters
pub(super) const DATA_NAMEDOBJECT_META_UPDATE_1: &'static str = r#"
ALTER TABLE `data_namedobject_meta` ADD COLUMN object_type SMALLINT DEFAULT 0;
ALTER TABLE `data_namedobject_meta` ADD COLUMN object_create_time INTEGER DEFAULT 0;
ALTER TABLE `data_namedobject_meta` ADD COLUMN author BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN dec_id BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN prev BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN body_prev_version BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN ref_objs BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN nonce BLOB DEFAULT NULL;
ALTER TABLE `data_namedobject_meta` ADD COLUMN difficulty BLOB DEFAULT 0;
"#;

// For all version upgrades, MAIN_TABLE_UPDATE_LIST[CURRENT_VERSION - 1] is the corresponding upgrade sql
pub(super) const MAIN_TABLE_UPDATE_LIST: [[&'static str; 1]; CURRENT_VERSION as usize] = [
    [DATA_NAMEDOBJECT_META_UPDATE_1]
];
