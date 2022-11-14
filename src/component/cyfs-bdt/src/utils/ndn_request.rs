use std::{
    path::{PathBuf}, 
};
use cyfs_base::*;
use crate::{
    ndn::{*, channel::{*, protocol::v0::*}}, 
    stack::{Stack}, 
};
use super::local_chunk_store::{LocalChunkWriter, LocalChunkListWriter};

// context -> referer 的过程

// referer -> ToString

// 从父context生成子context -》 知道如何转化成referer
// 知道所在的task group

// context id 


// referer 包含 acl上下文; 传输组上下文


// client
struct CyfsStack {
    
}


impl CyfsStack {
    fn get_data()
}


// ood 
struct CyfsStack

struct NdnDataRequester {
    stack: Stack,
    root_context: Context
}

impl NdnDataRequester {
    fn get_context(inner_path: String) -> BuckyResult<Context> {

    }

    fn get_data(
        &self, 
        object_id: ObjectId, 
        inner_path: String, 
        group: String
    ) -> BuckyResult<impl Seek + Read> {
        let context = self.get_context(inner_path)?;
        let task = create_download_task(object_id, group, context);
        task.reader()
    }
}


struct Context {
    parent: Context, 
}


impl Context {
    fn add_source(&self)
    fn get_source(&self)
}

struct DownloadGroup {
    
}



// gateway

struct NdnDataRequester {
    stack: Stack,
    root_context: Context
}

