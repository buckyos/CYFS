// 当前的数据库版本
pub(super) const CURRENT_VERSION: i32 = 0;

pub(super) const CREATE_MAIN_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS noc (
    object_id TEXT PRIMARY KEY NOT NULL UNIQUE, 
    protocol TINYTEXT,
    object_type SMALLINT,
    object_type_code TINYINT,
    device_id VARCHAR,

    dec_id VARCHAR,
    owner_id VARCHAR,
    author_id VARCHAR,

    create_time UNSIGNED BIG INT,
    update_time UNSIGNED BIG INT,
    insert_time UNSIGNED BIG INT,

    zone_seq UNSIGNED BIG INT DEFAULT 0,
    rank TINYINT,

    flags INTEGER,
    object BLOB NOT NULL
)"#;

pub(super) const MAIN_TABLE_ZONE_SEQ_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `main_table_zone_seq_index` on `noc` (`zone_seq`);
"#;

pub(super) const MAIN_TABLE_UPDATE_TIME_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `main_table_update_time_index` on `noc` (`update_time`);
"#;

pub(super) const INIT_NOC_SQL_LIST: [&'static str; 3] = [
    CREATE_MAIN_TABLE,
    MAIN_TABLE_ZONE_SEQ_INDEX,
    MAIN_TABLE_UPDATE_TIME_INDEX,
];

pub(super) const INSERT_NEW_SQL: &str = r#"
    insert into noc (
        object_id, 
        protocol, 
        object_type, 
        object_type_code, 
        device_id,

        dec_id,
        owner_id,
        author_id,
        create_time,
        update_time,    

        insert_time,
        rank,
        flags,
        object
        ) 
        values 
        (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14);
"#;

pub(super) const UPDATE_SQL: &str = r#"
    update noc set
        protocol = ?2,
        object_type = ?3,
        object_type_code =?4, 
        device_id =?5,

        dec_id =?6,
        owner_id = ?7,
        author_id = ?8,
        create_time = ?9,
        update_time = ?10,

        insert_time = ?11,
        rank=?12,
        flags = ?13,
        object = ?14
        
        where
        object_id = ?1 and update_time = ?15
"#;

pub(super) const UPDATE_SIGNS_SQL: &str = r#"
    update noc set
        insert_time = ?3,
        object = ?4
        
        where
        object_id = ?1 and insert_time = ?2
"#;

/*
// 需要注意新加的列只能在末尾！！！
// 版本0->1的升级
pub(super) const MAIN_TABLE_UPDATE_1_1: &'static str = r#"
ALTER TABLE `noc` ADD COLUMN zone_seq UNSIGNED BIG INT DEFAULT 0
"#;
pub(super) const MAIN_TABLE_UPDATE_1_2: &'static str = r#"
ALTER TABLE `noc` ADD COLUMN rank TINYINT
"#;

pub(super) const MAIN_TABLE_UPDATE_1: [&'static str; 3] =
    [MAIN_TABLE_UPDATE_1_1, MAIN_TABLE_UPDATE_1_2, MAIN_TABLE_ZONE_SEQ_INDEX];
*/

// 所有的版本升级, MAIN_TABLE_UPDATE_LIST[CURRENT_VERSION - 1]就是对应的升级sql
pub(super) const MAIN_TABLE_UPDATE_LIST: [[&'static str; 0]; CURRENT_VERSION as usize] = [];
