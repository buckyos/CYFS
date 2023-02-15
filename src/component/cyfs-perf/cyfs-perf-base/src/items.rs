use super::merge::PerfItemMerge;
use cyfs_base::*;
use cyfs_lib::*;

use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry, HashMap};
use std::fmt::{Display, Formatter};

// 统计所在的时间间隔
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfTimeRange {
    pub begin: u64,
    pub end: u64,
}

impl Display for PerfTimeRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.begin, self.end)
    }
}

impl Default for PerfTimeRange {
    fn default() -> Self {
        Self {
            begin: u64::MAX,
            end: u64::MIN,
        }
    }
}

impl PerfItemMerge<PerfTimeRange> for PerfTimeRange {
    fn merge(&mut self, other: Self) {
        self.begin = std::cmp::min(self.begin, other.begin);
        self.end = std::cmp::max(self.end, other.end);
    }
}

impl PerfTimeRange {
    pub fn now() -> Self {
        let now = bucky_time_now();
        Self {
            begin: now,
            end: now,
        }
    }

    // 更新结束时间
    pub fn update(&mut self) {
        self.end = bucky_time_now();
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfRequest {
    pub id: String,

    // 该request的统计区间
    pub time_range: PerfTimeRange,

    pub total: u32,
    pub success: u32,
    pub total_time: u64,
    pub total_size: Option<u64>,
}

impl std::fmt::Display for PerfRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id {} total {} success {}  total_time {} total_size {:?}",
            self.id, self.total, self.success, self.total_time, self.total_size
        )
    }
}

impl PerfItemMerge<PerfRequest> for PerfRequest {
    fn merge(&mut self, other: Self) {
        assert_eq!(self.id, other.id);

        self.time_range.merge(other.time_range);
        self.total += other.total;
        self.success += other.success;
        self.total_time += other.total_time;

        let total_size = self.total_size.take().unwrap_or(0) + other.total_size.unwrap_or(0);
        if total_size > 0 {
            self.total_size = Some(total_size);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfRequestBeginAction {
    pub tick: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct PerfRequestEndAction {
    id: String,
    err: u16, //BuckyErrorCode
    bytes: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
// 累计项，每次会在上一次的基础上累积结果
pub struct PerfAccumulation {
    pub id: String,
    pub time_range: PerfTimeRange,
    pub total: u32,
    pub success: u32,
    pub total_size: Option<u64>,
}

impl PerfItemMerge<PerfAccumulation> for PerfAccumulation {
    fn merge(&mut self, other: Self) {
        assert_eq!(self.id, other.id);

        self.time_range.merge(other.time_range);

        self.total += other.total;
        self.success += other.success;
        let total_size = self.total_size.take().unwrap_or(0) + other.total_size.unwrap_or(0);
        if total_size > 0 {
            self.total_size = Some(total_size);
        }
    }
}

impl std::fmt::Display for PerfAccumulation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id {} total {} success {}  total_size {:?}",
            self.id, self.total, self.success, self.total_size
        )
    }
}

// 更新一条记录，会覆盖上一条
// total表示当前数据总条目
// total_size表示当前总数量(总大小，总时间等)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfRecord {
    pub id: String,
    // 统计时刻
    pub time: u64,

    pub total: u64,
    pub total_size: Option<u64>,
}

impl PerfItemMerge<PerfRecord> for PerfRecord {
    fn merge(&mut self, other: Self) {
        assert_eq!(self.id, other.id);

        self.time = other.time;
        self.total = other.total;
        self.total_size = other.total_size;
    }
}

impl std::fmt::Display for PerfRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id {} total {} total_size {:?}",
            self.id, self.total, self.total_size
        )
    }
}

// 记录一次行为
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfAction {
    pub id: String,
    pub time: u64,
    pub err: u32,
    pub name: String,
    pub value: String,
}

impl std::fmt::Display for PerfAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id {} err {} name {} value {}",
            self.id, self.err, self.name, self.value
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// 一个统计实体
pub struct PerfIsolateEntity {
    pub id: String,

    pub time_range: PerfTimeRange,

    pub actions: Vec<PerfAction>,

    pub records: HashMap<String, PerfRecord>,

    pub accumulations: HashMap<String, PerfAccumulation>,

    pub reqs: HashMap<String, PerfRequest>,
}

impl std::fmt::Display for PerfIsolateEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id: {}, ", self.id)?;
        write!(f, "reqs: {:?}", self.reqs)
    }
}

impl PerfIsolateEntity {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_owned(),
            time_range: PerfTimeRange::default(),
            actions: vec![],
            records: HashMap::new(),
            accumulations: HashMap::new(),
            reqs: HashMap::new(),
        }
    }

    // 判断有没有有效数据
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
            && self.records.is_empty()
            && self.accumulations.is_empty()
            && self.reqs.is_empty()
    }
}

impl PerfItemMerge<PerfIsolateEntity> for PerfIsolateEntity {
    fn merge(&mut self, mut other: Self) {
        assert_eq!(self.id, other.id);

        // 统计区间也要合并
        self.time_range.merge(other.time_range);

        self.actions.append(&mut other.actions);
        self.records.merge(other.records);
        self.accumulations.merge(other.accumulations);
        self.reqs.merge(other.reqs);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerfIsolateEntityList {
    pub time_range: PerfTimeRange,
    pub list: HashMap<String, PerfIsolateEntity>,
}

declare_collection_codec_for_serde!(PerfIsolateEntityList);

impl PerfItemMerge<PerfIsolateEntityList> for PerfIsolateEntityList {
    fn merge(&mut self, other: Self) {
        self.time_range.merge(other.time_range);
        self.list.merge(other.list)
    }
}

impl PerfItemMerge<PerfIsolateEntity> for PerfIsolateEntityList {
    fn merge(&mut self, item: PerfIsolateEntity) {
        self.time_range.merge(item.time_range.clone());
        
        match self.list.entry(item.id.clone()) {
            Entry::Occupied(mut o) => o.get_mut().merge(item),
            Entry::Vacant(v) => {
                v.insert(item);
            }
        }
    }
}

impl Default for PerfIsolateEntityList {
    fn default() -> Self {
        Self {
            time_range: PerfTimeRange::default(),
            list: HashMap::new(),
        }
    }
}

impl PerfIsolateEntityList {
    pub fn clear(&mut self) {
        self.time_range = PerfTimeRange::default();
        self.list.clear();
    }

    pub fn is_empty(&self) -> bool {
        if self.list.is_empty() {
            return true;
        }

        // 如果所有子项为空，那么则为空
        self.list
            .iter()
            .fold(true, |acc, item| acc && item.0.is_empty())
    }
}
