
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};
use cyfs_base::*;
use super::resource::ResourcePerfData;
use crate::{PerfData, StatisticTask, StatisticTaskPtr, PerfDataAbstract};

pub trait SummaryStatisticTask: StatisticTask + std::fmt::Display + Send + Sync {
    fn add_total_size(&self, size: u64);
    fn progress(&self) -> usize;
}
pub type SummaryStatisticTaskPtr = Arc<dyn SummaryStatisticTask>;

pub struct SummaryStatisticTaskState {
    total_size: AtomicU64,
    stat: ResourcePerfData,
    task: Option<StatisticTaskPtr>,
}

impl SummaryStatisticTaskState {
    pub fn new(task_cb: Option<StatisticTaskPtr>) -> Self {
        Self {
            total_size: AtomicU64::new(0u64),
            stat: ResourcePerfData::default(),
            task: task_cb
        }
    }
}

impl std::fmt::Display for SummaryStatisticTaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(task) = &self.task {
            write!(f, "total_size={}, stat={}, task={}]", 
            self.total_size.load(Ordering::SeqCst),
            self.stat,
            task)
        } else {
            write!(f, "total_size={}, stat={}]", 
            self.total_size.load(Ordering::SeqCst),
            self.stat)
        }
    }
}

impl SummaryStatisticTask for SummaryStatisticTaskState {
    fn add_total_size(&self, size: u64) {
        self.total_size.fetch_add(size, Ordering::SeqCst);
    }

    fn progress(&self) -> usize {
        self.stat.progress()
    }
}

impl StatisticTask for SummaryStatisticTaskState {
    fn reset(&self) {
        if let Some(task) = &self.task {
            task.reset();
        }
    }


    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        if let Some(task) = &self.task {
            let r = task.on_stat(size).unwrap();

            self.stat.merge(&r);
            self.stat.set_progress({
                (100 * r.byte_total() / self.total_size.load(Ordering::SeqCst)) as  usize
            });

            Ok(r)
        } else {
            Ok(PerfData::default().clone_as_perfdata())
        }
    }

}

pub struct SummaryStatisticTaskImpl(SummaryStatisticTaskPtr);

impl SummaryStatisticTaskImpl {
    pub fn new(task_cb: Option<StatisticTaskPtr>) -> Self {
        Self(Arc::new(
            SummaryStatisticTaskState::new(task_cb)
        ))
    }

    pub fn ptr(&self) -> SummaryStatisticTaskPtr {
        self.0.clone()
    }

}

impl std::fmt::Display for SummaryStatisticTaskImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl SummaryStatisticTask for SummaryStatisticTaskImpl {
    fn add_total_size(&self, size: u64) {
        self.0.add_total_size(size)
    }

    fn progress(&self) -> usize {
        self.0.progress()
    }
}

impl StatisticTask for SummaryStatisticTaskImpl {
    fn reset(&self) {
        self.0.reset()
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        self.0.on_stat(size)
    }

}