use super::super::client::*;
use super::super::protocol::*;
use super::cache::SyncObjectsStateCache;
use super::data::DataSync;
use super::object_map_sync::*;
use super::sync_helper::*;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_chunk_cache::ChunkManager;

use std::sync::Arc;

#[derive(Debug)]
pub struct SyncDiffResult {
    pub revision: u64,

    // if target not exists, will return none
    pub target: Option<ObjectId>,
}

pub(crate) struct GlobalStateSyncClient {
    requestor: Arc<SyncClientRequestor>,
    state: GlobalStateSyncHelper,
    state_cache: SyncObjectsStateCache,
    bdt_stack: StackGuard,
    chunk_manager: Arc<ChunkManager>,
}

impl GlobalStateSyncClient {
    pub fn new(
        requestor: Arc<SyncClientRequestor>,
        state: GlobalStateSyncHelper,
        state_cache: SyncObjectsStateCache,
        bdt_stack: StackGuard,
        chunk_manager: Arc<ChunkManager>,
    ) -> Self {
        Self {
            requestor,
            state,
            state_cache,
            bdt_stack,
            chunk_manager,
        }
    }

    pub async fn sync(&self, mut req: SyncDiffRequest) -> BuckyResult<(bool, SyncDiffResult)> {
        // 先查看本地是否已经对应的旧版本的objectmap了
        let ret = self.state.load_target(&req).await;
        let current = match ret {
            Ok(Some((value, revision))) => {
                info!(
                    "sync diff with local version: dec={:?}, path={}, current={}, revison={}",
                    req.dec_id, req.path, value, revision
                );
                Some(value)
            }
            Ok(None) => {
                info!(
                    "sync diff without local version: dec={:?}, path={}",
                    req.dec_id, req.path,
                );
                None
            }
            Err(e) => {
                info!(
                    "sync diff but load local version error! dec={:?}, path={}, {}",
                    req.dec_id, req.path, e
                );
                None
            }
        };

        req.current = current;

        let resp = self.requestor.sync_diff(req.clone()).await.map_err(|e| {
            error!(
                "sync diff failed! dec={:?}, path={}, {}",
                req.dec_id, req.path, e
            );
            e
        })?;

        if resp.target.is_none() {
            warn!(
                "sync diff but target not found! dec={:?}, path={},",
                req.dec_id, req.path,
            );
            let ret = SyncDiffResult {
                revision: resp.revision,
                target: None,
            };

            return Ok((false, ret));
        }

        let target = resp.target.unwrap();

        let data_sync = DataSync::new(
            self.bdt_stack.clone(),
            self.chunk_manager.clone(),
            self.requestor.clone(),
            self.state_cache.clone(),
        );

        let sync = ObjectMapSync::new(
            target.clone(),
            self.state.new_op_env_cache(),
            self.state_cache.clone(),
            self.requestor.clone(),
            self.state.noc().clone(),
            self.state.device_id().clone(),
            data_sync,
        );

        let mut had_save_err = false;
        if resp.objects.len() > 0 {
            sync.save_objects(resp.objects, &mut had_save_err).await;
        }

        sync.sync(&mut had_save_err).await?;

        let ret = SyncDiffResult {
            revision: resp.revision,
            target: Some(target),
        };

        Ok((had_save_err, ret))
    }
}
