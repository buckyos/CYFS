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
    channel::{self, ChannelManager}, 
    chunk::{ChunkManager, ChunkManager2, ChunkReader}, 
    event::*, 
    root::RootTask,
};

#[derive(Clone)]
pub struct Config {
    pub atomic_interval: Duration, 
    pub schedule_interval: Duration, 
    pub channel: channel::Config,
}


struct StackImpl {
    stack: WeakStack, 
    last_schedule: AtomicU64, 
    chunk_manager: ChunkManager, 
    chunk_manager2: ChunkManager2, 
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
        let strong_stack = Stack::from(&stack);

        Self(Arc::new(StackImpl {
            stack: stack.clone(), 
            last_schedule: AtomicU64::new(0), 
            chunk_manager: ChunkManager::new(
                stack.clone(), 
                ndc.clone(), 
                tracker.clone(), 
                store.clone_as_reader()), 
            chunk_manager2: ChunkManager2::new(
                stack.clone(), 
                ndc, 
                tracker, 
                store), 
            channel_manager: ChannelManager::new(stack.clone()), 
            event_handler, 
            root_task: RootTask::new(100000, strong_stack.config().ndn.channel.history_speed.clone()),
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
            self.channel_manager().on_schedule(now);
            self.chunk_manager().on_schedule(now);
            self.root_task().on_schedule(now);
            self.0.last_schedule.store(now, Ordering::SeqCst);
        }
        self.channel_manager().on_time_escape(now);
    }
    
    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.0.chunk_manager
    }

    pub fn chunk_manager2(&self) -> &ChunkManager2 {
        &self.0.chunk_manager2
    }

    pub fn root_task(&self) -> &RootTask {
        &self.0.root_task
    }

    pub fn channel_manager(&self) -> &ChannelManager {
        &self.0.channel_manager
    }

    pub(super) fn event_handler(&self) -> &dyn NdnEventHandler {
        self.0.event_handler.as_ref()
    }

    
}

