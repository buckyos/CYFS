
use std::{vec::Vec, sync::atomic::{AtomicBool, Ordering}};

use cyfs_base::*;
use crate::{PerfDataAbstract, PerfData, StatisticTask, DynamicStatisticTask};
use crate::channel::protocol::v0::PieceData;

pub struct ChunkStatisticTask{
    data_caches: Vec<AtomicBool>,
    task: Option<DynamicStatisticTask>,
}

impl ChunkStatisticTask {
    pub fn new(max_index: u32,
               task: Option<DynamicStatisticTask>) -> Self {
        let mut caches = Vec::new();
        caches.resize_with(max_index as usize, || AtomicBool::new(false));

        Self {
            data_caches: caches,
            task: task,
        }
    }

    pub fn default(max_index: u32) -> Self {
        let mut caches = Vec::new();
        caches.resize_with(max_index as usize, || AtomicBool::new(false));

        Self {
            data_caches: caches,
            task: Some(DynamicStatisticTask::default()),
        }
    }

    pub fn on_data_stat(&self, data: &PieceData) -> BuckyResult<()> {
        if let Some(index) = data.desc.range_index(PieceData::max_payload() as u16) {
            if let Some(cache) = self.data_caches.get(index as usize) {
                if let Ok(_) = cache.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
                    if let Some(task) = &self.task {
                        let _ = task.on_stat(data.data.len() as u64);
                    }
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for ChunkStatisticTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(task) = &self.task {
            write!(f, "max_index: {}, task: {}", self.data_caches.len(), task)
        } else {
            write!(f, "max_index: {}", self.data_caches.len())
        }
    }
}

impl StatisticTask for ChunkStatisticTask {
    fn reset(&self) {
        for i in self.data_caches.iter() {
            i.store(false, Ordering::SeqCst);
        }
    }

    fn stat(&self) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        if let Some(task) = &self.task {
            task.stat()
        } else {
            Ok(PerfData::default().clone_as_perfdata())
        }
    }

    fn on_stat(&self, size: u64) -> BuckyResult<Box<dyn PerfDataAbstract>> {
        if let Some(task) = &self.task {
            task.on_stat(size)
        } else {
            Ok(PerfData::default().clone_as_perfdata())
        }
    }
}

