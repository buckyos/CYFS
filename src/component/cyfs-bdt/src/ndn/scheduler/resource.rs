use std::{
    sync::{RwLock, Mutex}, 
    collections::{LinkedList}, 
    time::Duration
};
use async_std::{
    sync::Arc
};

use cyfs_base::*;
use crate::{
    types::*
};

// struct ResourceQuotaImpl {
//     children: RwLock<BTreeMap<DeviceId, StatisticTaskPtr>>,
// }

// impl ResourceQuotaImpl {
//     fn add_child(&self, remote: &DeviceId, task: StatisticTaskPtr) -> BuckyResult<()> {
//         {
//             self.children.write().
//                           unwrap().
//                           entry(remote.clone()).
//                           or_insert(task.clone());
//         }
//         Ok(())
//     }

//     fn remove_child(&self, remote: &DeviceId) -> BuckyResult<()> {
//         {
//             let _ = self.children.write().unwrap().remove(remote);
//         }
//         Ok(())
//     }

//     fn get_child(&self, remote: &DeviceId) -> Option<StatisticTaskPtr> {
//         if let Some(task) = self.children.read().unwrap().get(remote) {
//             Some(task.clone())
//         } else {
//             None
//         }
//     }

//     fn count(&self) -> usize {
//         self.children.read().unwrap().len()
//     }
// }

// #[derive(Clone)]
// pub struct ResourceQuota(Arc<ResourceQuotaImpl>);

// impl ResourceQuota {
//     pub fn new() -> Self {
//         Self(Arc::new(ResourceQuotaImpl {
//             children: RwLock::new(BTreeMap::new()),
//         }))
//     }

//     pub fn add_child(&self, remote: &DeviceId, task: StatisticTaskPtr) -> BuckyResult<()> {
//         self.0.add_child(remote, task)
//     }

//     pub fn remove_child(&self, remote: &DeviceId) -> BuckyResult<()> {
//         self.0.remove_child(remote)
//     }

//     pub fn get_child(&self, remote: &DeviceId) -> Option<StatisticTaskPtr> {
//         self.0.get_child(remote)
//     }

//     pub fn count(&self) -> usize {
//         self.0.count()
//     }
// }

pub struct DurationedResourceUsage {
    start_at: Timestamp, 
    duration: Duration, 
    usage: ResourceUsage
}

impl DurationedResourceUsage {
    pub fn upstream(&self) -> u64 {
        self.usage.upstream
    }

    pub fn downstream(&self) -> u64 {
        self.usage.downstream
    }

    pub fn upstream_bandwidth(&self) -> u32 {
        (self.upstream() as f32 / self.duration.as_secs() as f32) as u32
    }

    pub fn downstream_bandwidth(&self) -> u32 {
        (self.downstream() as f32 / self.duration.as_secs() as f32) as u32
    }
}

#[derive(Clone)]
struct ResourceUsage {
    upstream: u64, 
    downstream: u64, 
}

impl ResourceUsage {
    fn new() -> Self {
        Self {
            upstream: 0, 
            downstream: 0,
        }
    }
    fn reset(&mut self) {
        self.upstream = 0;
        self.downstream = 0;
    }

    fn use_upstream(&mut self, len: usize) -> &mut Self {
        self.upstream += len as u64;
        self
    }

    fn use_downstream(&mut self, len: usize) -> &mut Self {
        self.downstream += len as u64;
        self
    }

    fn plus(&mut self, other: &Self) -> &mut Self {
        self.downstream += other.downstream;
        self.upstream += other.upstream;
        self 
    }

    fn divide(&mut self, by: usize) -> &mut Self {
        self.upstream = (self.upstream as f64 / by as f64) as u64;
        self.downstream = (self.downstream as f64 / by as f64) as u64;
        self
    }
}

#[derive(Clone)]
struct ResourceStatistic {
    start_at: Timestamp, 
    usage: ResourceUsage
}

impl ResourceStatistic {
    fn new(when: Timestamp) -> Self {
        Self {
            start_at: when, 
            usage: ResourceUsage::new()
        }
    }

    fn reset(&mut self, when: Timestamp) {
        self.start_at = when;
        self.usage.reset();
    }
}



// #[derive(Clone)]
// pub struct ResourceQuota {
//     memory: Option<u64>, 
//     downstream_bandwidth: Option<u32>, 
//     upstream_bandwidth: Option<u32>, 
//     cpu_usage: Option<u32> 
// }

// impl ResourceQuota {
//     fn new() -> Self {
//         Self {
//             memory: None, 
//             downstream_bandwidth: None, 
//             upstream_bandwidth: None, 
//             cpu_usage: None
//         }
//     }
// }

#[derive(Clone)]
struct Relation {
    owners: LinkedList<ResourceManager>, 
    children: LinkedList<ResourceManager>
}

struct StateImpl {
    aggregate_at: Timestamp, 
    latest: ResourceStatistic, 
    total: ResourceUsage
}

// 在多owner的情况下，owner遍历child时，child应当平分资源占用到不同的owner；
// owner分配child额度时，叠加多个owner分配的额度
struct ResourceManagerImpl {
    start_at: Timestamp, 
    relation: RwLock<Relation>, 
    statistic: Mutex<ResourceStatistic>, 
    state: RwLock<StateImpl>
}

#[derive(Clone)]
pub struct ResourceManager(Arc<ResourceManagerImpl>);

impl ResourceManager {
    fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl ResourceManager {
    pub fn new(owner: Option<ResourceManager>) -> Self {
        let now =  bucky_time_now();
        let resource = Self(Arc::new(
            ResourceManagerImpl {
                start_at: now, 
                relation: RwLock::new(Relation {
                    owners: LinkedList::new(), 
                    children: LinkedList::new()
                }), 
                statistic: Mutex::new(ResourceStatistic::new(now)),  
                state: RwLock::new(StateImpl {
                    // quote: ResourceQuota::new(), 
                    aggregate_at: now, 
                    latest: ResourceStatistic::new(now), 
                    total: ResourceUsage::new()
                })
            }
        ));
        if let Some(owner) = owner {
            let _ = owner.add_child(&resource);
        }
        resource
    }

    pub fn use_downstream(&self, len: usize) {
        self.0.statistic.lock().unwrap().usage.use_downstream(len);
    }

    pub fn use_upstream(&self, len: usize) {
        self.0.statistic.lock().unwrap().usage.use_upstream(len);
    }

    pub fn latest_usage(&self) -> DurationedResourceUsage {
        let state = self.0.state.read().unwrap();
        let duration = Duration::from_micros(state.aggregate_at - state.latest.start_at);
        DurationedResourceUsage {
            start_at: state.latest.start_at, 
            duration, 
            usage: state.latest.usage.clone()
        }
    }

    pub fn avg_usage(&self) -> DurationedResourceUsage {
        let state = self.0.state.read().unwrap();
        let duration = Duration::from_micros(state.aggregate_at - self.0.start_at);
        DurationedResourceUsage {
            start_at: self.0.start_at, 
            duration, 
            usage: state.total.clone()
        }
    }


    fn owner_count(&self) -> usize {
        self.0.relation.read().unwrap().owners.len()
    }

    // pub fn quota(&self) -> ResourceQuota {
    //     self.0.state.read().unwrap().quote.clone()
    // }

    pub fn aggregate(&self) {
        let children = self.0.relation.read().unwrap().children.clone();
        for c in children.clone() {
            c.aggregate();
        }


        let now = bucky_time_now();
        let mut latest = {
            let mut statistic = self.0.statistic.lock().unwrap();
            let latest = statistic.clone();
            statistic.reset(now);
            latest
        };
        
        let mut state = self.0.state.write().unwrap();
        for c in children {
            latest.usage.plus(&c.latest_usage().usage.clone().divide(c.owner_count()));
        }
        state.total.plus(&latest.usage);
        state.latest = latest;
       
        state.aggregate_at = now;
    }

    // pub fn schedule(&self, _quota: ResourceQuota) -> BuckyResult<()> {
    //     unimplemented!()
    // }

    pub fn add_child(&self, child: &ResourceManager) -> BuckyResult<()> {
        {
            let mut relation = self.0.relation.write().unwrap();
            relation.children.push_back(child.clone());
        }
        let mut relation = child.0.relation.write().unwrap();
        relation.owners.push_back(self.clone());
        Ok(())
    }

    pub fn remove_child(&self, child: &ResourceManager) -> BuckyResult<()> {
        let _ = {
            self.aggregate();
            let mut relation = self.0.relation.write().unwrap();
            if let Some((i, _)) = relation.children.iter().enumerate().find(|(_, r)| r.ptr_eq(child)) {
                let mut last_part = relation.children.split_off(i);
                let _ = last_part.pop_front();
                relation.children.append(&mut last_part);
                Ok(())
            } else {
                Err(BuckyError::new(BuckyErrorCode::NotFound, "not a child"))
            }
        }?;
        let mut relation = child.0.relation.write().unwrap();
        if let Some((i, _)) = relation.owners.iter().enumerate().find(|(_, r)| r.ptr_eq(self)) {
            let mut last_part = relation.owners.split_off(i);
            let _ = last_part.pop_front();
            relation.owners.append(&mut last_part);
            Ok(())
        } else {
            unreachable!()
        }
    }
}


#[async_std::test]
async fn resource_aggregate() {
    use async_std::{task};

    let owner = ResourceManager::new(None);
    let child = ResourceManager::new(Some(owner.clone()));

    child.use_downstream(1000);
    let _ = task::sleep(Duration::from_secs(1)).await;
    owner.aggregate();
    assert!(child.latest_usage().downstream_bandwidth() > 0);
    assert!(child.avg_usage().downstream_bandwidth() > 0);
    assert!(owner.latest_usage().downstream_bandwidth() > 0);
    assert!(owner.avg_usage().downstream_bandwidth() > 0);
}