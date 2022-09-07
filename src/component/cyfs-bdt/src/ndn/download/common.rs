use std::{
    collections::{LinkedList}, 
    sync::{Arc, RwLock}
};
use cyfs_base::*;
use super::super::{
    channel::*
};

#[derive(Clone)]
pub struct DownloadSource {
    pub target: DeviceId, 
    pub object_id: Option<ObjectId>, 
    pub encode_desc: PieceSessionType, 
    pub referer: Option<String>
}



struct SingleContextImpl {
    referer: Option<String>, 
    sources: RwLock<LinkedList<DownloadSource>>, 
}

#[derive(Clone)]
pub struct SingleDownloadContext(Arc<SingleContextImpl>);

impl SingleDownloadContext {
    pub fn streams(referer: Option<String>, remotes: Vec<DeviceId>) -> Self {
        let mut sources = LinkedList::new();
        for remote in remotes {
            sources.push_back(DownloadSource {
                target: remote, 
                object_id: None, 
                encode_desc: PieceSessionType::Stream(0), 
                referer: None
            });
        } 
        Self(Arc::new(SingleContextImpl {
            referer, 
            sources: RwLock::new(sources)
        }))
    }

    pub fn referer(&self) -> Option<&str> {
        self.0.referer.as_ref().map(|s| s.as_str())
    }

    pub fn add_source(&self, source: DownloadSource) {
        self.0.sources.write().unwrap().push_back(source);
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


struct MultiContextImpl {
    contexts: RwLock<LinkedList<SingleDownloadContext>>
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
        self.0.contexts.write().unwrap().push_back(context);
    }

    pub fn sources_of(&self, filter: impl Fn(&DownloadSource) -> bool + Copy, limit: usize) -> LinkedList<DownloadSource> {
        let mut result = LinkedList::new();
        let mut limit = limit;
        let contexts = self.0.contexts.read().unwrap();
        for context in contexts.iter() {
            let mut part = context.sources_of(filter, limit);
            limit -= part.len();
            result.append(&mut part);
            if limit == 0 {
                break;
            }
        }   
        result
    }
}

