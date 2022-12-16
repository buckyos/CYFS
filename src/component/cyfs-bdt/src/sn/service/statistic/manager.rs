
use std::{sync::{RwLock, Arc}, collections::{BTreeMap, }, time::Duration};

use cyfs_base::{DeviceId, bucky_time_now, SocketAddr, };
use cyfs_util::SqliteStorage;
use once_cell::sync::OnceCell;

use crate::Timestamp;

use super::{PeerStatus, };

const CACHE_MAX_TIMEOUT: Duration = Duration::from_secs(5);

struct StatisticImpl {
}

struct StatisticManagerImpl {
    storage: Option<Arc<SqliteStorage>>,
    last_cache_timestamp: Timestamp,
    statistics: BTreeMap<String, PeerStatus>,
}

#[derive(Clone)]
pub struct StatisticManager(Arc<RwLock<StatisticManagerImpl>>);

impl StatisticManager {
    fn new() -> Self {
        let ret = Self(Arc::new(RwLock::new(StatisticManagerImpl{
            storage: None,
            last_cache_timestamp: bucky_time_now(),
            statistics: BTreeMap::new(),
        })));

        let arc_ret = ret.clone();
        async_std::task::spawn(async move {
            let mut storage = SqliteStorage::new();
            match storage.init("sn-statistic").await {
                Ok(_) => {
                    arc_ret.0.write().unwrap()
                        .storage = Some(Arc::new(storage));
                }
                Err(err) => {
                    error!("failed to init statistic-db with err = {}", err);
                }
            }
        });

        ret
       
    }
}

impl StatisticManager {
    pub fn get_instance() -> &'static Self {
        static INSTANCE: OnceCell<StatisticManager> = OnceCell::new();
        INSTANCE.get_or_init(|| Self::new())
    }

    pub fn get_peer_status(&self, id: DeviceId, now: Timestamp) -> PeerStatus {
        self.0.write().unwrap()
            .statistics
            .entry(id.to_string())
            .or_insert(PeerStatus::with_peer(id, now))
            .clone()
    }

    pub fn get_endpoint_status(&self, endpoint: SocketAddr) -> PeerStatus {
        self.0.write().unwrap()
            .statistics
            .entry(endpoint.to_string())
            .or_insert(PeerStatus::with_endpoint(endpoint))
            .clone()
    }
}

impl StatisticManager {
    pub fn on_time_escape(&self, now: Timestamp) {
        let (all, storage) = {
            let manager = &mut *self.0.write().unwrap();

            if now > manager.last_cache_timestamp &&
               now - manager.last_cache_timestamp >= CACHE_MAX_TIMEOUT.as_micros() as u64 {
                let all: Vec<PeerStatus> = manager.statistics.values().cloned().collect();
                manager.last_cache_timestamp = now;
                (Some(all), manager.storage.clone())
            } else {
                (None, None)
            }
        };

        if let Some(storage) = storage {
            let storage = unsafe {&mut *(Arc::as_ptr(&storage) as *mut SqliteStorage)};
            async_std::task::spawn(async move {
                if let Some(all) = all {
                    for a in all.iter() {
                        a.storage(storage).await;
                    }
                }
        
            });
        }
    }
}

mod test {

    #[test]
    fn test() {
        use cyfs_base::{DeviceId, bucky_time_now};

        use crate::TempSeq;
    
        use super::StatisticManager;

        let m = StatisticManager::get_instance();

        let s1 = m.get_peer_status(DeviceId::default(), bucky_time_now());
        // s1.online(bucky_time_now()+10);
        s1.online(TempSeq::default(), bucky_time_now()+10);

        std::thread::sleep(std::time::Duration::from_secs(1));


        let _s2 = m.get_peer_status(DeviceId::default(), bucky_time_now());

        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
