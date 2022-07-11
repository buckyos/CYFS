use cyfs_base::*;
use cyfs_perf_base::*;

#[cfg(feature = "mongo")]
use crate::perf_db::*;

use crate::storage::*;

use std::sync::Mutex;
use lazy_static::lazy_static;
use std::collections::HashMap;

pub struct PerfManager {
    storage: Option<Box<dyn Storage>>,
}

impl Clone for PerfManager {
    fn clone(&self) -> Self {
        let storage = self.storage.as_ref().unwrap();
        let storage = (*storage).clone();

        Self {
            storage: Some(storage),
        }
    }
}

impl PerfManager {
    pub fn new(
    ) -> PerfManager {
        PerfManager {
            storage: None,
        }
    }

    pub async fn init(&mut self, storage_type: &StorageType, isolate: &str) -> BuckyResult<()> {

        assert!(self.storage.is_none());

        let storage = self.create_storage(&storage_type, isolate).await?;
        self.storage = Some(storage);
        Ok(())
    }

    async fn create_storage(
        &self,
        storage_type: &StorageType,
        ioslate: &str,
    ) -> BuckyResult<Box<dyn Storage>> {
        let ret = match storage_type {
            #[cfg(feature = "mongo")]
            StorageType::MangoDB => {
                Box::new(MangodbStorage::new(ioslate).await?)
                    as Box<dyn Storage>
            }
        };

        Ok(ret)
    }

    /*
        request
    */
    #[cfg(debug_assertions)]
    pub async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let this = self.clone();
        let all = all.to_owned();
        async_std::task::spawn(async move {
            this.insert_entity_list_with_event(people_id, device_id, dec_id, dec_name, version, &all).await
        }).await
    }

    #[cfg(not(debug_assertions))]
    pub async fn insert_entity_list(&self, people_id: String, device_id: String, dec_id: String, dec_name: String, version: String, all: &HashMap<String, PerfIsolateEntity>) -> BuckyResult<()> {
        let this = self.clone();
        let all = all.to_owned();
        this.insert_entity_list_with_event(people_id, device_id, dec_id, dec_name, version, &all).await
    }

    pub async fn insert_entity_list_with_event(
        &self,
        people_id: String, 
        device_id: String, 
        dec_id: String,
        dec_name: String,
        version: String,
        all: &HashMap<String, PerfIsolateEntity>
    ) -> BuckyResult<()> {
        let all = all.clone();
    
        let ret = self
            .storage
            .as_ref()
            .unwrap()
            .insert_entity_list(people_id, device_id, dec_id, dec_name, version, &all)
            .await;

        ret
    }

 
}

lazy_static! {
    pub static ref PERF_MANAGER: Mutex<PerfManager> = {
        return Mutex::new(PerfManager::new());
    };
}