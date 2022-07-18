use std::{
    time::Duration, 
    sync::{atomic::{AtomicU64, Ordering}},
};
use async_std::{
    sync::Arc, 
    task,
    future
};
use cyfs_base::*;
use cyfs_util::{
    cache::*, 
};
use crate::{
    types::*, 
    stack::{WeakStack, Stack}, 
    utils::{mem_tracker::MemTracker, local_chunk_store::LocalChunkReader}
};
use super::{
    scheduler::*, 
    channel::{self, ChannelManager}, 
    chunk::{ChunkManager, ChunkReader}, 
    event::*, 
    root::RootTask,
    task::DirConfig
};

#[derive(Clone)]
pub struct Config {
    pub atomic_interval: Duration, 
    pub schedule_interval: Duration, 
    pub channel: channel::Config,
    pub limit: LimitConfig,
    pub dir: DirConfig,
}


struct StackImpl {
    stack: WeakStack, 
    last_schedule: AtomicU64, 
    resource: ResourceManager, 
    chunk_manager: ChunkManager, 
    channel_manager: ChannelManager, 
    event_handler: Box<dyn NdnEventHandler>, 
    root_task: RootTask,
}

#[derive(Clone)]
pub struct NdnStack(Arc<StackImpl>);

impl NdnStack {
    pub(crate) fn open(
        stack: WeakStack, 
        ndc: Option<Box<dyn NamedDataCache>>,
        tracker: Option<Box<dyn TrackerCache>>, 
        store: Option<Box<dyn ChunkReader>>, 
        event_handler: Option<Box<dyn NdnEventHandler>>, 
    ) -> Self {
       
        let mem_tracker = MemTracker::new();
        let tracker = tracker.unwrap_or(TrackerCache::clone(&mem_tracker));
        let ndc = ndc.unwrap_or(NamedDataCache::clone(&mem_tracker));
        let store = store.unwrap_or(Box::new(LocalChunkReader::new(ndc.as_ref(), tracker.as_ref())));
        let event_handler = event_handler.unwrap_or(Box::new(DefaultNdnEventHandler::new()));
        
        let resource = ResourceManager::new(None);
        Self(Arc::new(StackImpl {
            stack: stack.clone(), 
            last_schedule: AtomicU64::new(0), 
            resource: resource.clone(), 
            chunk_manager: ChunkManager::new(
                stack.clone(), 
                resource.clone(), 
                ndc, 
                tracker, 
                store), 
            channel_manager: ChannelManager::new(stack.clone()), 
            event_handler, 
            root_task: RootTask::new(resource.clone()),
        }))
    }

    pub(crate) fn start(&self) {
        let stack = Stack::from(&self.0.stack);
        let atomic_interval = stack.config().ndn.atomic_interval;
        {
            let ndn = self.clone();
            task::spawn(async move {
                loop {
                    ndn.on_time_escape(bucky_time_now());
                    let _ = future::timeout(atomic_interval, future::pending::<()>()).await;
                }
            });
        }   
    }

    fn on_time_escape(&self, now: Timestamp) {
        let stack = Stack::from(&self.0.stack);
        let last_schedule = self.0.last_schedule.load(Ordering::SeqCst);
        if now > last_schedule 
            && Duration::from_millis(now - last_schedule) > stack.config().ndn.schedule_interval {
            self.collect_resource_usage();
            self.schedule_resource();
            self.apply_scheduled_resource();
            self.0.last_schedule.store(now, Ordering::SeqCst);
        }
        self.channel_manager().on_time_escape(now);
    }
    
    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.0.chunk_manager
    }

    pub fn root_task(&self) -> &RootTask {
        &self.0.root_task
    }

    pub(crate) fn channel_manager(&self) -> &ChannelManager {
        &self.0.channel_manager
    }

    pub(super) fn event_handler(&self) -> &dyn NdnEventHandler {
        self.0.event_handler.as_ref()
    }


}

impl Scheduler for NdnStack {
    fn collect_resource_usage(&self) {
        self.chunk_manager().collect_resource_usage();
        self.root_task().collect_resource_usage();
    }

    fn schedule_resource(&self) {
        self.chunk_manager().schedule_resource();
        self.root_task().schedule_resource();
    }

    fn apply_scheduled_resource(&self) {
        self.chunk_manager().apply_scheduled_resource();
        self.root_task().apply_scheduled_resource();
    }
}

