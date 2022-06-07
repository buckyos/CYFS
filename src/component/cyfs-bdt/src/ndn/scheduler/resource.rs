use std::{
    sync::{RwLock, atomic::{AtomicU64, AtomicI32, AtomicU32, Ordering::*}}, 
    collections::{LinkedList, BTreeMap}, 
    time::Duration
};
use async_std::{
    sync::Arc
};

use cyfs_base::*;
use crate::{
    types::*
};
use super::StatisticTaskPtr;

struct ResourceQuotaImpl {
    children: RwLock<BTreeMap<DeviceId, StatisticTaskPtr>>,
}

impl ResourceQuotaImpl {
    fn add_child(&self, remote: &DeviceId, task: StatisticTaskPtr) -> BuckyResult<()> {
        {
            self.children.write().
                          unwrap().
                          entry(remote.clone()).
                          or_insert(task.clone());
        }
        Ok(())
    }

    fn remove_child(&self, remote: &DeviceId) -> BuckyResult<()> {
        {
            let _ = self.children.write().unwrap().remove(remote);
        }
        Ok(())
    }

    fn get_child(&self, remote: &DeviceId) -> Option<StatisticTaskPtr> {
        if let Some(task) = self.children.read().unwrap().get(remote) {
            Some(task.clone())
        } else {
            None
        }
    }

    fn count(&self) -> usize {
        self.children.read().unwrap().len()
    }
}

#[derive(Clone)]
pub struct ResourceQuota(Arc<ResourceQuotaImpl>);

impl ResourceQuota {
    pub fn new() -> Self {
        Self(Arc::new(ResourceQuotaImpl {
            children: RwLock::new(BTreeMap::new()),
        }))
    }

    pub fn add_child(&self, remote: &DeviceId, task: StatisticTaskPtr) -> BuckyResult<()> {
        self.0.add_child(remote, task)
    }

    pub fn remove_child(&self, remote: &DeviceId) -> BuckyResult<()> {
        self.0.remove_child(remote)
    }

    pub fn get_child(&self, remote: &DeviceId) -> Option<StatisticTaskPtr> {
        self.0.get_child(remote)
    }

    pub fn count(&self) -> usize {
        self.0.count()
    }
}

#[derive(Clone)]
pub struct ResourceUsage {
    memory: u64, 
    downstream_bandwidth: u32, 
    upstream_bandwidth: u32, 
    cpu_usage: f32
}

impl ResourceUsage {
    fn new() -> Self {
        Self {
            memory: 0, 
            downstream_bandwidth: 0, 
            upstream_bandwidth: 0, 
            cpu_usage: 0.0
        }
    }

    fn plus(&mut self, other: &Self) -> &mut Self {
        self.memory += other.memory;
        self.downstream_bandwidth += other.downstream_bandwidth;
        self.upstream_bandwidth += other.upstream_bandwidth;
        self.cpu_usage += other.cpu_usage;
        self 
    }

    fn divide(&mut self, by: usize) -> &mut Self {
        self.memory = self.memory / by as u64;
        self.upstream_bandwidth = self.upstream_bandwidth / by as u32;
        self.downstream_bandwidth = self.downstream_bandwidth / by as u32;
        self.cpu_usage = self.cpu_usage / by as f32;
        self
    }
}

struct ResourceStatistic {
    start: AtomicU64, 
    memory: AtomicI32, 
    downstream: AtomicU32, 
    upstream: AtomicU32, 
    cpu_epoch: AtomicU64
}

impl ResourceStatistic {
    fn new() -> Self {
        Self {
            start: AtomicU64::new(0), 
            memory: AtomicI32::new(0), 
            downstream: AtomicU32::new(0), 
            upstream: AtomicU32::new(0), 
            cpu_epoch: AtomicU64::new(0), 
        }
    }

    fn alloc_memory(&self, len: usize) {
        self.memory.fetch_add(len as i32, SeqCst);
    }

    fn free_memory(&self, len: usize) {
        self.memory.fetch_sub(len as i32, SeqCst);
    }

    fn use_downstream(&self, len: usize) {
        self.downstream.fetch_add(len as u32, SeqCst);
    }

    fn use_upstream(&self, len: usize) {
        self.upstream.fetch_add(len as u32, SeqCst);
    }

    fn use_cpu(&self, len: u64) {
        self.cpu_epoch.fetch_add(len, SeqCst);
    } 

    fn aggregate(&self, now: Timestamp, usage: &mut ResourceUsage) {
        let memory = self.memory.swap(0, SeqCst);
        let downstream = self.downstream.swap(0, SeqCst);
        let upstream = self.upstream.swap(0, SeqCst);
        let cpu_epoch = self.cpu_epoch.swap(0, SeqCst);
        let start = self.start.swap(now, SeqCst);
        let duration = Duration::from_micros(now - start);
        if memory > 0 {
            usage.memory += memory as u64;
        } else {
            let free = (-memory) as u64;
            if usage.memory > free {
                usage.memory -= free;
            } else {
                usage.memory = 0;
            } 
        }
        usage.downstream_bandwidth = (downstream as f32 / duration.as_secs_f32()) as u32;
        usage.upstream_bandwidth = (upstream as f32 / duration.as_secs_f32()) as u32;
        usage.cpu_usage = cpu_epoch as f32 / duration.as_micros() as f32;
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
    quote: ResourceQuota, 
    aggregate_at: Timestamp, 
    self_usage: ResourceUsage, 
    total_usage: ResourceUsage
}

// 在多owner的情况下，owner遍历child时，child应当平分资源占用到不同的owner；
// owner分配child额度时，叠加多个owner分配的额度
struct ResourceManagerImpl {
    relation: RwLock<Relation>, 
    statistic: ResourceStatistic, 
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
        let resource = Self(Arc::new(
            ResourceManagerImpl {
                relation: RwLock::new(Relation {
                    owners: LinkedList::new(), 
                    children: LinkedList::new()
                }), 
                statistic: ResourceStatistic::new(), 
                state: RwLock::new(StateImpl {
                    quote: ResourceQuota::new(), 
                    aggregate_at: 0, 
                    self_usage: ResourceUsage::new(), 
                    total_usage: ResourceUsage::new()
                })
            }
        ));
        if let Some(owner) = owner {
            let _ = owner.add_child(&resource);
        }
        resource
    }

    pub fn alloc_memory(&self, len: usize) {
        self.0.statistic.alloc_memory(len)
    }

    pub fn free_memory(&self, len: usize) {
        self.0.statistic.free_memory(len)
    }

    pub fn use_downstream(&self, len: usize) {
        self.0.statistic.use_downstream(len)
    }

    pub fn use_upstream(&self, len: usize) {
        self.0.statistic.use_upstream(len)
    }

    pub fn use_cpu(&self, len: u64) {
        self.0.statistic.use_cpu(len)
    }

    pub fn usage(&self) -> ResourceUsage {
        self.0.state.read().unwrap().total_usage.clone()
    }

    fn owner_count(&self) -> usize {
        self.0.relation.read().unwrap().owners.len()
    }

    pub fn quota(&self) -> ResourceQuota {
        self.0.state.read().unwrap().quote.clone()
    }

    pub fn aggregate(&self) {
        let now = bucky_time_now();
        let children = self.0.relation.read().unwrap().children.clone();

        let mut state = self.0.state.write().unwrap();
        self.0.statistic.aggregate(now, &mut state.self_usage);
        
        let mut total = state.self_usage.clone();
        total.plus(&state.self_usage);
        for c in children {
            total.plus(&c.usage().divide(c.owner_count()));
        }
        state.total_usage = total;
        state.aggregate_at = now;
    }

    pub fn schedule(&self, _quota: ResourceQuota) -> BuckyResult<()> {
        unimplemented!()
    }

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