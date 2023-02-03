use cyfs_base::*;

use rusqlite::{Connection, OpenFlags};
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use thread_local::ThreadLocal;

pub struct SqliteConnectionHolder {
    data_file: PathBuf,

    /* SQLite does not support multiple writers. */
    read_conn: Arc<ThreadLocal<RefCell<Connection>>>,
    write_conn: Arc<ThreadLocal<RefCell<Connection>>>,
    conn_rw_lock: RwLock<u32>,
}

impl SqliteConnectionHolder {
    pub fn new(data_file: PathBuf) -> Self {
        Self {
            data_file,
            read_conn: Arc::new(ThreadLocal::new()),
            write_conn: Arc::new(ThreadLocal::new()),
            conn_rw_lock: RwLock::new(0),
        }
    }

    pub fn get_write_conn(
        &self,
    ) -> BuckyResult<(std::cell::RefMut<Connection>, RwLockWriteGuard<u32>)> {
        let conn = self.write_conn.get_or_try(|| {
            let ret = self.create_new_conn(false)?;
            Ok::<RefCell<Connection>, BuckyError>(RefCell::new(ret))
        })?;

        let lock = self.conn_rw_lock.write().unwrap();
        Ok((conn.borrow_mut(), lock))
    }

    pub fn get_read_conn(&self) -> BuckyResult<(std::cell::Ref<Connection>, RwLockReadGuard<u32>)> {
        let conn = self.read_conn.get_or_try(|| {
            let ret = self.create_new_conn(true)?;
            Ok::<RefCell<Connection>, BuckyError>(RefCell::new(ret))
        })?;

        let lock = self.conn_rw_lock.read().unwrap();
        Ok((conn.borrow(), lock))
    }

    fn create_new_conn(&self, read_only: bool) -> BuckyResult<Connection> {
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_URI
        } else {
            OpenFlags::default()
        };

        let conn = Connection::open_with_flags(&self.data_file, flags).map_err(|e| {
            let msg = format!("open noc db failed, db={}, {}", self.data_file.display(), e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::SqliteError, msg)
        })?;

        // 设置一个30s的锁重试
        if let Err(e) = conn.busy_timeout(std::time::Duration::from_secs(30)) {
            error!("init sqlite busy_timeout error! {}", e);
        }

        Ok(conn)
    }
}
