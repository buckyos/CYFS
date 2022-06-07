use cyfs_base::{DeviceId, ChunkId, BuckyResult, BuckyError};
use lazy_static::lazy_static;
use async_std::sync::{Mutex};
use rusqlite::{Connection, Row, Params};
use std::path::{PathBuf};
use std::str::FromStr;

pub struct ChunkMeta {
    database: Option<PathBuf>,
}

/// SQLite接口封装
/// 
/// 包含下面一组接口
/// * init 初始化
/// * query 查询
/// * insert 插入
/// * query_row 获取一行
/// 
impl ChunkMeta {
    pub fn new() -> ChunkMeta { ChunkMeta { database:None }}

    pub fn init(&mut self, database: &PathBuf, create_table_list: &Vec<String>) -> BuckyResult<()> {
        self.database = Some(PathBuf::from(database));
        
        let conn = Connection::open(self.database.as_ref().unwrap()).map_err(|e|{
            e
        })?;

        for create_table in create_table_list {
            if let Err(e) = conn.execute(create_table, []) {
                // conn.close();
                return Err(BuckyError::from(e));
            }
        }
        
        Ok(())
    }

    pub fn query<P>(&self, sql: &str, params: P) -> BuckyResult<()>
        where
        P: Params
    {
        let conn = Connection::open(self.database.as_ref().unwrap()).map_err(|e|{
            error!("open db:{} failed, e:{}", self.database.as_ref().unwrap().to_string_lossy(), e.to_string());
            e
        })?;

        conn.execute(sql, params).map_err(|e|{
            error!("[db] sql:{}, msg: {}", sql, e.to_string());
            e
        })?;

        Ok(())
    }

    pub fn insert<P>(&self, sql: &str, params: P) -> BuckyResult<i64> where P: Params
    {
        let conn = Connection::open(self.database.as_ref().unwrap()).map_err(|e|{
            error!("open db:{} failed, e:{}", self.database.as_ref().unwrap().to_string_lossy(), e.to_string());
            e
        })?;

        conn.execute(sql, params).map_err(|e|{
            error!("[db] sql:{}, msg: {}", sql, e.to_string());
            e
        })?;

        let id = conn.last_insert_rowid();

        Ok(id)
    }

    pub fn query_row<T, P, F>(&self, sql: &str, params: P, f: F) -> BuckyResult<T>
        where
            P: Params,
            F: FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = Connection::open(self.database.as_ref().unwrap()).map_err(|e|{
            error!("open db:{} failed, e:{}", self.database.as_ref().unwrap().to_string_lossy(), e.to_string());
            e
        })?;
        
        let result = conn.query_row(sql, params, f).map_err(|e|{
            error!("[db] sql:{}, msg: {}", sql, e.to_string());
            e
        })?;

        Ok(result)
    }

    pub fn query_map<T, P, F>(&self, sql: &str, params: P, f: F) -> BuckyResult<Vec<T>>
        where
        P: Params,
        F: FnMut(&Row) -> rusqlite::Result<T>
    {
        let conn = Connection::open(self.database.as_ref().unwrap()).map_err(|e|{
            error!("open db:{} failed, e:{}", self.database.as_ref().unwrap().to_string_lossy(), e.to_string());
            e
        })?;
        
        let mut stat = conn.prepare(sql)?;
        let rows = stat.query_map(params, f).map_err(|e|{
            error!("[db] sql:{}, msg: {}", sql, e.to_string());
            e
        })?;

        let mut values = Vec::new();
        for row in rows {
            values.push(row?);
        }

        Ok(values)
    }
}

pub fn get_device_id(row: &rusqlite::Row, idx: usize)-> rusqlite::Result<DeviceId>{
    let v:String = row.get(idx)?;
    let device_id = DeviceId::from_str(&v).map_err(|e|{
        error!("parse peer id failed, err:{}, field value:{}", e, v);
        rusqlite::Error::from(rusqlite::types::FromSqlError::InvalidType)
    })?;
    Ok(device_id)
}

pub fn get_chunk_id(row: &rusqlite::Row, idx: usize)-> rusqlite::Result<ChunkId>{
    let v:String = row.get(idx)?;
    let chunk_id = ChunkId::from_str(&v).map_err(|e|{
        error!("parse chunk id failed, err:{}", e);
        rusqlite::Error::from(rusqlite::types::FromSqlError::InvalidType)
    })?;
    Ok(chunk_id)
}

lazy_static! {
    pub static ref CHUNK_META: Mutex<ChunkMeta> = {
        return Mutex::new(ChunkMeta::new());
    };
}