

const CREATE_TRACKER_MAIN_TABLE: &'static str = r#"
CREATE TABLE IF NOT EXISTS tracker (
    id TEXT NOT NULL,
    
    pos TEXT NOT NULL,
    pos_type TINYINT NOT NULL,
    direction TINYINT NOT NULL,

    insert_time UNSIGNED BIG INT NOT NULL,
    update_time UNSIGNED BIG INT NOT NULL,

    flags INTEGER DEFAULT 0,

    PRIMARY KEY(id, pos, pos_type, direction)
)"#;


pub(super) const INIT_TRACKER_SQL_LIST: [&'static str; 1] = [
    CREATE_TRACKER_MAIN_TABLE,
];