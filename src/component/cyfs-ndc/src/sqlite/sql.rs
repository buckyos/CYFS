pub(crate) const FILE_MAIN_TABLE: &'static str = "file";
pub(crate) const FILE_QUICKHASN_TABLE: &'static str = "file_quick_hash";
pub(crate) const FILE_DIRS_TABLE: &'static str = "file_dirs";

pub(crate) const CHUNK_MAIN_TABLE: &'static str = "chunk";
pub(crate) const CHUNK_TRANS_TABLE: &'static str = "chunk_trans";
pub(crate) const CHUNK_REF_TABLE: &'static str = "chunk_ref";

// file主表
const CREATE_FILE_MAIN_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS file (
    hash TEXT PRIMARY KEY NOT NULL UNIQUE, 
    file_id TEXT NOT NULL UNIQUE,
    length UNSIGNED BIG INT NOT NULL,
    owner TEXT DEFAULT NULL,

    insert_time UNSIGNED BIG INT NOT NULL,
    update_time UNSIGNED BIG INT NOT NULL,

    flags INTEGER DEFAULT 0
)"#;

const FILE_MAIN_TABLE_FILE_ID_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `file_main_table_file_id_index` on `file` (`file_id`);
"#;

// file的快速hash索引
// quick_hash+length联合主键，可能对应多个file_id
const CREATE_FILE_QUICKHASH_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS file_quick_hash (
    hash TEXT NOT NULL,
    length UNSIGNED BIG INT NOT NULL,
    file_id TEXT NOT NULL,
    PRIMARY KEY(hash, length)
)"#;

const FILE_QUICKHASH_TABLE_FILE_ID_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `file_quick_hash_table_file_id_index` on `file_quick_hash` (`file_id`);
"#;

// file的dirs归属表
// file_id为主键，一个file_id可以关联多个(dir_id+inner_path)对
const CREATE_FILE_DIRS_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS file_dirs (
    file_id TEXT NOT NULL,
    dir_id TEXT NOT NULL,
    inner_path TEXT NOT NULL,
    PRIMARY KEY(file_id, dir_id, inner_path)
)"#;

const FILE_DIRS_TABLE_DIR_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `file_dirs_table_dir_index` on `file_dirs` (`dir_id`);
"#;

// chunk主表
const CREATE_CHUNK_MAIN_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS chunk (
    chunk_id TEXT PRIMARY KEY NOT NULL UNIQUE, 

    insert_time UNSIGNED BIG INT NOT NULL,
    update_time UNSIGNED BIG INT NOT NULL,
    last_access_time UNSIGNED BIG INT NOT NULL,

    state TINYINT NOT NULL,
    flags INTEGER DEFAULT 0
)"#;

// chunk和trans_session关系表，多对多
const CREATE_CHUNK_TRANS_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS chunk_trans (
    chunk_id TEXT NOT NULL, 
    trans_id TEXT NOT NULL,
    PRIMARY KEY(chunk_id, trans_id)
)"#;

const CHUNK_TRANS_TABLE_TRANS_ID_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `chunk_trans_table_trans_id_index` on `chunk_trans` (`trans_id`);
"#;

// chunk和对象的关系表，多对多关系
const CREATE_CHUNK_REF_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS chunk_ref (
    chunk_id TEXT NOT NULL, 
    object_id TEXT NOT NULL,
    relation TINYINT NOT NULL,
    PRIMARY KEY(chunk_id, object_id, relation)
)"#;

const CHUNK_REF_TABLE_OBJECT_ID_INDEX: &'static str = r#"
CREATE INDEX IF NOT EXISTS `chunk_ref_table_object_id_index` on `chunk_ref` (`object_id`);
"#;

pub(super) const INIT_FILE_SQL_LIST: [&'static str; 6] = [
    CREATE_FILE_MAIN_TABLE,
    FILE_MAIN_TABLE_FILE_ID_INDEX,

    CREATE_FILE_QUICKHASH_TABLE,
    FILE_QUICKHASH_TABLE_FILE_ID_INDEX,

    CREATE_FILE_DIRS_TABLE,
    FILE_DIRS_TABLE_DIR_INDEX,
];

pub(super) const INIT_CHUNK_SQL_LIST: [&'static str; 5] = [
    CREATE_CHUNK_MAIN_TABLE,

    CREATE_CHUNK_TRANS_TABLE,
    CHUNK_TRANS_TABLE_TRANS_ID_INDEX,
    
    CREATE_CHUNK_REF_TABLE,
    CHUNK_REF_TABLE_OBJECT_ID_INDEX,
];