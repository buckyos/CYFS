use log::*;
use std::{
    path::{Path, PathBuf}, 
    collections::{BTreeSet}, 
    sync::RwLock
};
use async_std::sync::Arc;
use rusqlite;
use cyfs_base::*;
use cyfs_bdt::pn::service::{ProxyDeviceStub, ProxyServiceEvents};

pub struct Config {
    pub bandwidth: Vec<(u32, usize/*limit*/)> 
}

impl Config {
    fn limit_of(&self, bandwidth: u32) -> Option<usize> {
        self.bandwidth.iter().find_map(|(bw, limit)| {
            if *bw == bandwidth {
                Some(*limit)
            } else {
                None
            }
        })
    }
}

struct StorageImpl {
    path: PathBuf, 
    config: Config, 
    white_list: RwLock<BTreeSet<DeviceId>>
}

#[derive(Clone)]
pub struct Storage(Arc<StorageImpl>);

impl Storage {
    pub fn new(path: &Path, config: Config) -> BuckyResult<Self> {
        let storage = Self(Arc::new(StorageImpl {
            path: path.to_owned(), 
            config, 
            white_list: RwLock::new(Default::default())
        }));
        let _ = storage.init()?;
        Ok(storage)
    }

    fn conn(&self) -> BuckyResult<rusqlite::Connection> {
        let conn = rusqlite::Connection::open(self.0.path.as_path())?;
        Ok(conn)
    }

    fn init(&self) -> BuckyResult<()> {
        let conn = self.conn()?;
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS auth (
            device TEXT UNIQUE NOT NULL PRIMARY KEY,
            bandwidth INTEGER NOT NULL
        );", rusqlite::NO_PARAMS)?;
        Ok(())
    }

    pub fn rent(&self, device: DeviceId, bandwidth: u32) -> BuckyResult<()> {
        let limit = self.0.config.limit_of(bandwidth)
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "bandwith not in config"))?;
        let conn = self.conn()?;

        // 这里不需要一致性
        let used: u32 = conn.query_row(
            "SELECT count(device) FROM auth WHERE bandwidth=?;",  
            [bandwidth],
            |r| r.get(0))?;
        let used = used as usize;
        if used >= limit {
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "device on pn reach limit"));
        }
        match conn.execute("INSERT INTO auth (device, bandwidth) VALUES (?, ?);", 
            &[&device.to_string(), &bandwidth.to_string()]) {
            Ok(_) => Ok(()),
            Err(err) => {
                error!("sql err {}", err);
                Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "device id has rent pn"))
            }
        }
    }

    pub fn add_white_list(&self, device: DeviceId) -> BuckyResult<()> {
        if !self.0.white_list.write().unwrap().insert(device) {
            Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "device id has in white list"))
        } else {
            Ok(())
        }
    }

    pub fn cancel(&self, device: DeviceId, bandwidth: u32) -> BuckyResult<()> {
        if self.0.white_list.write().unwrap().remove(&device) {
            return Ok(());
        }
        let conn = self.conn()?;
        conn.execute("DELETE FROM auth WHERE device=? AND bandwidth=?;", 
            &[&device.to_string(), &bandwidth.to_string()])?;
        Ok(())
    }

    pub fn contract_of(&self, device: &DeviceId) -> BuckyResult<Vec<u32>> {
        if self.0.white_list.read().unwrap().contains(device) {
            Ok(vec![self.0.config.bandwidth[0].0])
        } else {
            let conn = self.conn()?;
            match conn.query_row("SELECT bandwidth FROM auth WHERE device=?;", 
                &[&device.to_string()], 
                    |r| r.get(0)) {
                Ok(bw) => Ok(vec![bw]), 
                Err(err) => {
                    match err {
                        rusqlite::Error::QueryReturnedNoRows => Ok(vec![]), 
                        _ => Err(err.into())
                    }
                }
            }
        }
        
    }

    pub fn used(&self) -> BuckyResult<Vec<(u32, usize/*used*/, usize/*limit*/)>> {
        let conn = self.conn()?;
        let mut result = vec![];
        for (bw, limit) in self.0.config.bandwidth.iter() {
            let used: u32 = conn.query_row(
                "SELECT count(device) FROM auth WHERE bandwidth=?;",  
                &[&bw.to_string()], 
                |r| r.get(0))?;
            let used = used as usize;
            let used = std::cmp::min(used, *limit);
            result.push((*bw, used, *limit));
        }
        Ok(result)
        
    }
}

#[async_trait::async_trait]
impl ProxyServiceEvents for Storage {
    async fn pre_create_tunnel(
        &self, 
        _mix_key: &AesKey, 
        device_pair: &(ProxyDeviceStub, ProxyDeviceStub)) -> BuckyResult<()> {

        if !self.contract_of(&device_pair.0.id)?.is_empty() 
            || !self.contract_of(&device_pair.1.id)?.is_empty() {
            Ok(())
        } else {
            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, "not allowed"))
        }
    }
}
