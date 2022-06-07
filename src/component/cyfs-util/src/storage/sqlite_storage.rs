use async_trait::async_trait;
use cyfs_base::BuckyError;
use crate::get_cyfs_root_path;
use rusqlite::Connection;
use std::error::Error;
use std::path::PathBuf;

const TABLE_NAME: &str = "profile";

pub struct SqliteStorage {
    inited: bool,
    path: PathBuf,
    dirty: bool,
    //conn: Option<Connection>,
}

impl SqliteStorage {
    pub fn new() -> SqliteStorage {
        SqliteStorage {
            inited: false,
            path: PathBuf::from(""),
            dirty: false,
            //conn: None,
        }
    }

    pub async fn init(&mut self, service_name: &str) -> Result<(), Box<dyn Error>> {
        assert!(!self.inited);
        self.inited = true;

        let dir = get_cyfs_root_path().join("profile").join(service_name);
        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!("create profile dir error! dir={}, err={}", dir.display(), e);
                error!("{}", msg);

                return Err(Box::<dyn Error>::from(msg));
            }
        }

        let file = dir.join("profile.db");
        self.path = file;
        debug!(
            "sqlite storage service: {}, file path: {}",
            service_name,
            self.path.display()
        );
        if !self.path.exists() {
            info!("sqlite storage file not exists! db={}", self.path.display());
            self.create_db()?;
        }
        // let conn = Connection::open(self.path.as_path()).map_err(|e| {
        //     warn!("open db failed, db={}, e={}", self.path.display(), e);
        //     e
        // })?;
        //self.conn = Some(conn);

        Ok(())
    }

    fn create_db(&mut self) -> Result<(), Box<dyn Error>> {
        let conn = Connection::open(self.path.as_path()).map_err(|e| {
            warn!("open db failed, db={}, e={}", self.path.display(), e);
            e
        })?;
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                key TEXT PRIMARY KEY NOT NULL UNIQUE, 
                value BLOB NOT NULL
            );",
            TABLE_NAME
        );
        conn.execute(&sql, []).map_err(|e| e)?;
        Ok(())
    }
}

#[async_trait]
impl super::AsyncStorage for SqliteStorage {
    async fn set_item(&mut self, key: &str, value: String) -> Result<(), BuckyError> {
        assert!(self.inited);
        //let conn = self.conn.as_ref().unwrap();
        let conn = Connection::open(self.path.as_path()).map_err(|e| {
            warn!("open db failed, db={}, e={}", self.path.display(), e);
            e
        })?;
        let sql = format!("REPLACE INTO {} (key, value) VALUES (?1, ?2)", TABLE_NAME);
        conn.execute(&sql, rusqlite::params![key, value])
            .map_err(|e| {
                warn!("[sqlite] set item failed, e={}", e);
                e
            })?;
        Ok(())
    }

    async fn get_item(&self, key: &str) -> Option<String> {
        assert!(self.inited);
        let conn = Connection::open(self.path.as_path())
            .map_err(|e| {
                warn!("open db failed, db={}, e={}", self.path.display(), e);
                e
            })
            .ok()?;
        let sql = format!("SELECT value FROM {} WHERE key=?1", TABLE_NAME);
        match conn.query_row(&sql, rusqlite::params![key], |row| {
            let value: String = row.get(0)?;
            Ok(value)
        }) {
            Ok(value) => Some(value),
            Err(_e) => None,
        }
    }

    async fn remove_item(&mut self, key: &str) -> Option<()> {
        assert!(self.inited);
        let conn = Connection::open(self.path.as_path())
            .map_err(|e| {
                warn!("open db failed, db={}, e={}", self.path.display(), e);
                e
            })
            .ok()?;
        let sql = format!("DELETE FROM {} WHERE key=?1", TABLE_NAME);
        let r = conn.execute(&sql, rusqlite::params![key]).map_err(|e| {
            warn!("[sqlite] remove item failed, key={}, e={}", key, e);
        });
        debug!("remove item return: {:?}", r);

        if r.unwrap() > 0 {
            Some(())
        } else {
            None
        }
    }

    async fn clear(&mut self) {
        assert!(self.inited);
        if let Ok(conn) = Connection::open(self.path.as_path()) {
            let sql = format!("DELETE FROM {}", TABLE_NAME);
            let _r = conn.execute(&sql, []).map_err(|e| {
                warn!("[sqlite] clear failed e={}", e);
            });
        }
    }

    async fn clear_with_prefix(&mut self, prefix: &str) {
        assert!(self.inited);
        if let Ok(conn) = Connection::open(self.path.as_path()) {
            let sql = format!("DELETE FROM {} WHERE key LIKE ?1", TABLE_NAME);
            let _r = conn
                .execute(&sql, rusqlite::params![format!("{}%", prefix)])
                .map_err(|e| {
                    warn!(
                        "[sqlite] clear with prefix failed, prefix={}, e={}",
                        prefix, e
                    );
                });
        }
    }
}
