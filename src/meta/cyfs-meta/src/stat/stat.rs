use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_std::prelude::StreamExt;
use cyfs_base::*;
use async_trait::async_trait;
use crate::stat::sqlite_storage::{SqliteConfig, SqliteStorage};
use chrono::{DateTime, Utc};
use log::{error, warn};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct StatConfig {
    pub memory_stat: bool,
    pub sqlite: Option<SqliteConfig>
}

#[derive(Default, Clone)]
pub struct MemoryStat {
    pub new_people: u32,
    pub new_device: u32,
    pub active_people: HashSet<ObjectId>,
    pub active_device: HashSet<ObjectId>,
    pub api_fail: HashMap<String, u32>,
    pub api_success: HashMap<String, u32>,
}

#[derive(Default, Clone)]
pub struct StatCache {
    pub add_desc_stat: Vec<(ObjectId, DateTime<Utc>)>,
    pub api_call: Vec<(String, u16, DateTime<Utc>)>,
    pub query_desc: Vec<(ObjectId, bool, DateTime<Utc>)>,
}

pub struct StatInner {
    storage: Box<dyn Storage + Sync + Send>,
    enable_memory_stat: bool,
    memory_stat: Mutex<MemoryStat>,
    stat_cache: Mutex<StatCache>,
}

impl StatInner {
    async fn save(&self) {
        let mut cache = StatCache::default();
        std::mem::swap(self.stat_cache.lock().unwrap().deref_mut(), &mut cache);

        if self.enable_memory_stat {
            let mut memory = self.memory_stat.lock().unwrap();
            for (id, _) in &cache.add_desc_stat {
                match id.obj_type_code() {
                    ObjectTypeCode::People => {
                        memory.new_people += 1;
                    }
                    ObjectTypeCode::Device => {
                        memory.new_device += 1;
                    }
                    _ => {}
                }
            }

            for (name, ret, _) in &cache.api_call {
                if *ret == 0 {
                    memory.api_success.entry(name.clone()).and_modify(|u| *u += 1).or_insert(1);
                } else {
                    memory.api_fail.entry(name.clone()).and_modify(|u| *u += 1).or_insert(1);
                }
            }

            for (id, exist, _) in &cache.query_desc {
                if *exist {
                    match id.obj_type_code() {
                        ObjectTypeCode::People => {
                            memory.active_people.insert(id.clone());
                        }
                        ObjectTypeCode::Device => {
                            memory.active_device.insert(id.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        let _ = self.storage.save(cache).await.map_err(|e| {
            warn!("save stat err {}", e);
            e
        });
    }
}

#[derive(Clone)]
pub struct Stat(Arc<StatInner>);

/*
统计数据：
每日新增用户：people和device的新增量
每日活跃用户：people和device的查询量，每个查询相当于活跃。这个可以按天记
api调用结果：主要看错误的情况。这个也可以按天记
*/

impl Stat {
    pub fn new(config: StatConfig) -> Self {
        let storage: Box<dyn Storage + Send + Sync> = if let Some(option) = config.sqlite {
            Box::new(SqliteStorage::new(option))
        } else {
            Box::new(FakeStorage {})
        };
        let inner = StatInner {
            storage,
            enable_memory_stat: config.memory_stat,
            memory_stat: Mutex::new(MemoryStat::default()),
            stat_cache: Mutex::new(StatCache::default()),
        };

        Self {
            0: Arc::new(inner),
        }
    }

    pub fn start(&self) {
        // 开始统计。每分钟存储一次，临时数据存内存，再存一份简单统计到内存
        let inner = self.0.clone();
        async_std::task::spawn(async move {
            if let Err(e) = inner.storage.init().await {
                error!("init stat storage err {}", e);
                // return;
            }
            let mut interval = async_std::stream::interval(Duration::from_secs(60));

            while let Some(_) = interval.next().await {
                let _ = inner.save().await;
            }
        });
    }

    pub fn add_desc(&self, id: &ObjectId) {
        // 新增统计，先只统计每天新增People和Device的个数
        let code = id.obj_type_code();
        if code == ObjectTypeCode::People || code == ObjectTypeCode::Device {
            self.0.stat_cache.lock().unwrap().add_desc_stat.push((id.clone(), Utc::now()));
        }
    }

    pub fn api_call(&self, name: &str, result: u16) {
        // api统计？先记录调用结果吧
        self.0.stat_cache.lock().unwrap().api_call.push((name.to_owned(), result, Utc::now()));
    }

    pub fn query_desc(&self, id: &ObjectId, exist: bool) {
        // get desc统计，统计id和查询结果
        self.0.stat_cache.lock().unwrap().query_desc.push((id.clone(), exist, Utc::now()));
    }


}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn init(&self) -> BuckyResult<()>;

    async fn save(&self, cache: StatCache) -> BuckyResult<()>;
}

struct FakeStorage {}

#[async_trait]
impl Storage for FakeStorage {
    async fn init(&self) -> BuckyResult<()> {
        Ok(())
    }

    async fn save(&self, _: StatCache) -> BuckyResult<()> {
        Ok(())
    }
}