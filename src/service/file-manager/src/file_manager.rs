use lazy_static::lazy_static;
use async_std::sync::{Mutex};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use cyfs_base::{BuckyResult, AnyNamedObject, RawConvertTo, ObjectId, RawFrom};

pub struct FileManager {
    database: Option<PathBuf>,
}

const CREATE_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS "file_desc" (
        "id"	TEXT NOT NULL UNIQUE,
        "desc" BLOB NOT NULL,
        PRIMARY KEY("id")
    );
"#;

const INSERT: &str = r#"
    INSERT OR REPLACE INTO file_desc VALUES (?1, ?2);
"#;

const SELECT: &str = r#"
    SELECT desc from file_desc where id=?1;
"#;

impl FileManager {
    pub fn new() -> FileManager { FileManager { database:None }}

    pub fn init(&mut self, database: &Path) -> BuckyResult<()> {
        self.database = Some(PathBuf::from(database));
        let conn = Connection::open(self.database.as_ref().unwrap())?;
        conn.execute(CREATE_TABLE, [])?;
        Ok(())
    }

    pub async fn set(&self, id: &ObjectId, desc: &AnyNamedObject) -> BuckyResult<()> {
        let data = desc.to_vec()?;
        let conn = Connection::open(self.database.as_ref().unwrap())?;
        conn.execute(INSERT, params![id.to_string(), data])?;
        Ok(())
    }

    pub async fn get(&self, id: &ObjectId) -> BuckyResult<AnyNamedObject> {
        let conn = Connection::open(self.database.as_ref().unwrap())?;
        let desc_buf = conn.query_row(SELECT, params![id.to_string()], |row| -> rusqlite::Result<Vec<u8>>{
            Ok(row.get(0)?)
        })?;
        let desc = AnyNamedObject::clone_from_slice(&desc_buf)?;
        Ok(desc)
    }
}

lazy_static! {
    pub static ref FILE_MANAGER: Mutex<FileManager> = {
        return Mutex::new(FileManager::new());
    };
}