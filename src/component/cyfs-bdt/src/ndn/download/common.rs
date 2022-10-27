use std::{
    collections::{LinkedList}, 
    sync::{Arc, RwLock}, 
};
use cyfs_base::*;
use crate::{
    stack::Stack, 
    types::*
};
use super::super::{
    types::*
};

#[derive(Clone)]
pub struct DownloadSource {
    pub target: DeviceDesc, 
    pub object_id: Option<ObjectId>, 
    pub encode_desc: ChunkEncodeDesc, 
    pub referer: Option<String>
}



struct SingleContextImpl {
    referer: Option<String>, 
    sources: RwLock<LinkedList<DownloadSource>>, 
}

#[derive(Clone)]
pub struct SingleDownloadContext(Arc<SingleContextImpl>);

impl Default for SingleDownloadContext {
    fn default() -> Self {
        Self(Arc::new(SingleContextImpl {
            referer: None, 
            sources: RwLock::new(Default::default()), 
        }))
    }
}

impl SingleDownloadContext {
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn new(referer: Option<String>) -> Self {
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(Default::default())
        }))
    }

    pub fn desc_streams(referer: Option<String>, remotes: Vec<DeviceDesc>) -> Self {
        let mut sources = LinkedList::new();
        for remote in remotes {
            sources.push_back(DownloadSource {
                target: remote, 
                object_id: None, 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
                referer: None
            });
        } 
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        }))
    }

    pub async fn id_streams(stack: &Stack, referer: Option<String>, remotes: Vec<DeviceId>) -> BuckyResult<Self> {
        let mut sources = LinkedList::new();
        for remote in remotes {
            let device = stack.device_cache().get(&remote).await
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::NotFound, "device desc not found"))?;
            sources.push_back(DownloadSource {
                target: device.desc().clone(), 
                object_id: None, 
                encode_desc: ChunkEncodeDesc::Stream(None, None, None), 
                referer: None
            });
        } 
        Ok(Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        })))
    }

    pub fn referer(&self) -> Option<&str> {
        self.0.referer.as_ref().map(|s| s.as_str())
    }

    pub fn add_source(&self, source: DownloadSource) {
        self.0.sources.write().unwrap().push_back(source);
    }

    pub fn source_exists(&self, source: &DownloadSource) -> bool {
        let sources = self.0.sources.read().unwrap();
        sources.iter().find(|s| s.target.device_id() == source.target.device_id() && s.encode_desc.support_desc(&source.encode_desc)).is_some()
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut count = 0;
        let sources = self.0.sources.read().unwrap();
        for source in sources.iter() {
            if filter(source) {
                result.push_back(DownloadSource {
                    target: source.target.clone(), 
                    object_id: source.object_id.clone(), 
                    encode_desc: source.encode_desc.clone(), 
                    referer: source.referer.clone().or(self.referer().map(|s| s.to_owned()))
                });
                count += 1;
                if count >= limit {
                    return result;
                }
            }
        }
        return result;
    }
}


#[derive(Clone, Copy, Eq, PartialEq)]
enum ContextState {
    Normal, 
    Paused
}

struct ContextCount {
    normal_count: usize, 
    paused_count: usize
}

impl ContextCount {
    fn state(&self) -> ContextState {
        if self.normal_count > 0 {
            ContextState::Normal 
        } else {
            ContextState::Paused
        }
    }

    fn task_count(&self) -> usize {
        self.normal_count + self.paused_count
    }
}

struct ContextStub {
    context: SingleDownloadContext, 
    count: ContextCount
}

struct MultiContextImpl {
    contexts: RwLock<LinkedList<ContextStub>>
}

#[derive(Clone)]
pub struct MultiDownloadContext(Arc<MultiContextImpl>);

impl MultiDownloadContext {
    pub fn new() -> Self {
        Self(Arc::new(MultiContextImpl {
            contexts: RwLock::new(LinkedList::new())
        }))
    }

    pub fn add_context(&self, context: SingleDownloadContext) {
        let mut contexts = self.0.contexts.write().unwrap();
        if let Some(stub) = contexts.iter_mut().find(|s| s.context.ptr_eq(&context)) {
            stub.count.normal_count += 1;
        } else {
            contexts.push_back(ContextStub {
                context, 
                count: ContextCount {
                    normal_count: 1, 
                    paused_count: 0
                }
            });
        }
    }


    pub fn remove_context(&self, context: &SingleDownloadContext, state: DownloadTaskState) {
        let mut contexts = self.0.contexts.write().unwrap();
        
        let to_remove = if let Some((index, stub)) = contexts.iter_mut().enumerate().find(|(_, stub)| stub.context.ptr_eq(context)) {
            match state {
                DownloadTaskState::Paused => if stub.count.paused_count > 0 {
                    stub.count.paused_count -= 1;
                }, 
                _ => if stub.count.paused_count > 0 {
                    stub.count.normal_count -= 1;
                }, 
            }
            if stub.count.task_count() == 0 {
                Some(index)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(index) = to_remove {
            let mut back_parts = contexts.split_off(index);
            let _ = back_parts.pop_front();
            contexts.append(&mut back_parts);
            // contexts.remove(index);
        }
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool + Copy, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut limit = limit;
        let contexts = self.0.contexts.read().unwrap();
        for stub in contexts.iter() {
            if stub.count.state() == ContextState::Normal {
                let mut part = stub.context.sources_of(filter, limit);
                limit -= part.len();
                result.append(&mut part);
                if limit == 0 {
                    break;
                }
            } 
        }   
        result
    }

    pub fn source_exists(&self, source: &DownloadSource) -> bool {
        self.0.contexts.read().unwrap().iter().find(|stub| stub.context.source_exists(source)).is_some()
    }
}


#[derive(Clone, Copy)]
pub enum DownloadTaskPriority {
    Backgroud = 1, 
    Normal = 2, 
    Realtime = 4,
}

impl Default for DownloadTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}


// 对scheduler的接口
#[derive(Debug)]
pub enum DownloadTaskState {
    Downloading(u32/*速度*/, f32/*进度*/),
    Paused,
    Error(BuckyErrorCode/*被cancel的原因*/), 
    Finished
}

#[derive(Clone, Debug)]
pub enum DownloadTaskControlState {
    Normal, 
    Paused, 
    Canceled, 
}


pub trait DownloadTask: Send + Sync {
    fn context(&self) -> &SingleDownloadContext;
    fn clone_as_task(&self) -> Box<dyn DownloadTask>;
    fn state(&self) -> DownloadTaskState;
    fn control_state(&self) -> DownloadTaskControlState;

    fn resume(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }
    fn cancel(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }
    fn pause(&self) -> BuckyResult<DownloadTaskControlState> {
        Ok(DownloadTaskControlState::Normal)
    }

    fn priority_score(&self) -> u8 {
        DownloadTaskPriority::Normal as u8
    }
    fn add_task(&self, _path: Option<String>, _sub: Box<dyn DownloadTask>) -> BuckyResult<()> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "no implement"))
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn DownloadTask>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32;
    fn cur_speed(&self) -> u32;
    fn history_speed(&self) -> u32;

    fn drain_score(&self) -> i64 {
        0
    }
    fn on_drain(&self, expect_speed: u32) -> u32;
}