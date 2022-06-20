use std::{convert::TryFrom, fmt::Debug, sync::Arc};
use cyfs_base::*;
use cyfs_lib::*;
use crate::{
    contracts::*, 
    contract_client::*
};

struct ClientImpl {
    stack: Arc<SharedCyfsStack>, 
}

#[derive(Clone)]
pub struct DsgCacheClient(Arc<ClientImpl>);

impl DsgCacheClient {
    pub fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self(Arc::new(ClientImpl {
            stack,
        }))
    }

    pub async fn add_chunks(&self, chunks: &Vec<ChunkId>) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn remove_chunks(&self, chunks: &Vec<ChunkId>) -> BuckyResult<()> {
        unimplemented!()
    }

    pub async fn has_chunk(&self, chunk: &ChunkId) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub async fn bind_contract(&self, contract: ObjectId) -> BuckyResult<()> {
        unimplemented!()
    }
}



