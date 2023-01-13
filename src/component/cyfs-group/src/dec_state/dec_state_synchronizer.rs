// the manager of the DEC's state that synchronized from the group's rpath

use std::collections::HashSet;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject, ObjectDesc, ObjectId,
};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{
    helper::{verify_block, Timer},
    network::NonDriver,
    storage::{DecStorage, DecStorageCache},
    CHANNEL_CAPACITY, SYNCHRONIZER_TIMEOUT,
};

enum DecStateSynchronizerMessage {
    ProposalResult(
        ObjectId,
        BuckyResult<(
            Option<NONObjectInfo>,
            GroupConsensusBlock,
            GroupConsensusBlock,
        )>,
    ),
    StateChange(GroupConsensusBlock, GroupConsensusBlock),
}

pub struct DecStateSynchronizer {
    local_id: ObjectId,
    tx_dec_state_sync_message: async_std::channel::Sender<(DecStateSynchronizerMessage, ObjectId)>,
}

impl DecStateSynchronizer {
    pub fn new(
        local_id: ObjectId,
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        non_driver: crate::network::NonDriver,
        store: DecStorage,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);

        let mut runner = DecStateSynchronizerRunner::new(local_id, rx, store);

        async_std::task::spawn(async move { runner.run().await });

        Self {
            local_id,
            tx_dec_state_sync_message: tx,
        }
    }

    pub async fn on_proposal_complete(
        &self,
        proposal_id: ObjectId,
        result: BuckyResult<(
            Option<NONObjectInfo>,
            GroupConsensusBlock,
            GroupConsensusBlock,
        )>,
        remote: ObjectId,
    ) {
    }

    pub async fn on_state_change(
        &self,
        header_block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
        remote: ObjectId,
    ) {
    }
}

struct DecStateSynchronizerCache {
    state_cache: DecStorageCache,
    group_chunk_id: ObjectId,
    group: Group,
}

struct UpdateNotifyInfo {
    header_block: GroupConsensusBlock,
    qc_block: GroupConsensusBlock,
    remotes: HashSet<ObjectId>,
    group_chunk_id: ObjectId,
    group: Group,
}

struct DecStateSynchronizerRunner {
    local_id: ObjectId,
    rpath: GroupRPath,
    rx_dec_state_sync_message:
        async_std::channel::Receiver<(DecStateSynchronizerMessage, ObjectId)>,
    timer: Timer,
    store: DecStorage,
    state_cache: Option<DecStateSynchronizerCache>,
    update_notifies: Option<UpdateNotifyInfo>,

    non_driver: NonDriver,
}

impl DecStateSynchronizerRunner {
    fn new(
        local_id: ObjectId,
        rpath: GroupRPath,
        rx_dec_state_sync_message: async_std::channel::Receiver<(
            DecStateSynchronizerMessage,
            ObjectId,
        )>,
        store: DecStorage,
        non_driver: NonDriver,
    ) -> Self {
        Self {
            local_id,
            rpath,
            rx_dec_state_sync_message,
            timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            store,
            state_cache: None,
            update_notifies: None,
            non_driver,
        }
    }

    async fn handle_proposal_complete(
        &mut self,
        proposal_id: ObjectId,
        result: BuckyResult<(
            Option<NONObjectInfo>,
            GroupConsensusBlock,
            GroupConsensusBlock,
        )>,
        remote: ObjectId,
    ) {
        let exe_result = match result {
            Ok((result, header_block, qc_block)) => {
                if header_block
                    .proposals()
                    .iter()
                    .find(|p| p.proposal == proposal_id)
                    .is_none()
                {
                    return;
                }
                if !header_block.check() {
                    return;
                }

                if self
                    .push_update_notify(header_block, qc_block, remote)
                    .await
                    .is_err()
                {
                    return;
                }

                if self.sync_state().await.is_err() {
                    return;
                }
                Ok(result)
            }
            Err(e) => Err(e),
        };

        // TODO: notify the app
    }

    async fn handle_state_change(
        &mut self,
        header_block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
        remote: ObjectId,
    ) {
        self.push_update_notify(header_block, qc_block, remote)
            .await;
    }

    async fn sync_state(&mut self) -> BuckyResult<()> {
        unimplemented!()
    }

    async fn check_cache(&mut self) -> &Option<DecStateSynchronizerCache> {
        if self.state_cache.is_none() {
            let state_cache = self.store.cur_state().await;
            if let Some(state_cache) = state_cache {
                let group_chunk_id = state_cache.header_block.group_chunk_id().clone();
                let group = self
                    .non_driver
                    .get_group(self.rpath.group_id(), Some(&group_chunk_id))
                    .await;
                if let Ok(group) = group {
                    self.state_cache = Some(DecStateSynchronizerCache {
                        state_cache,
                        group_chunk_id,
                        group,
                    });
                }
            }
        }

        &self.state_cache
    }

    async fn push_update_notify(
        &mut self,
        header_block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        if qc_block.qc().is_none() {
            log::warn!(
                "the qc is none for qc-block({})",
                qc_block.named_object().desc().object_id()
            );
            return Err(BuckyError::new(BuckyErrorCode::Unknown, "qc lost"));
        }

        if let Some(notify) = self.update_notifies.as_mut() {
            match notify.header_block.height().cmp(&header_block.height()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    notify.remotes.insert(remote);
                    return Ok(());
                }
                std::cmp::Ordering::Greater => return Ok(()),
            }
        }

        let group = match self.update_notifies.as_ref() {
            Some(n) => Some((n.group_chunk_id, n.group.clone())),
            None => self
                .check_cache()
                .await
                .as_ref()
                .map(|c| (c.group_chunk_id, c.group.clone())),
        }
        .map_or(None, |(chunk_id, group)| {
            if &chunk_id == header_block.group_chunk_id() {
                Some((chunk_id, group))
            } else {
                None
            }
        });

        // group changed
        let group = match group {
            Some(group) => group,
            None => {
                let group = self
                    .non_driver
                    .get_group(self.rpath.group_id(), Some(header_block.group_chunk_id()))
                    .await?;
                (header_block.group_chunk_id().clone(), group)
            }
        };

        if verify_block(&header_block, qc_block.qc().as_ref().unwrap(), &group.1).await? {
            self.update_notifies = Some(UpdateNotifyInfo {
                header_block: header_block,
                qc_block: qc_block,
                remotes: HashSet::from([remote]),
                group_chunk_id: group.0,
                group: group.1,
            });
        };

        Ok(())
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                message = self.rx_dec_state_sync_message.recv().fuse() => match message {
                    Ok((DecStateSynchronizerMessage::ProposalResult(proposal, result), remote)) => self.handle_proposal_complete(proposal, result, remote).await,
                    Ok((DecStateSynchronizerMessage::StateChange(block, qc_block), remote)) => self.handle_state_change(block, qc_block, remote).await,
                    Err(e) => {
                        log::warn!("[dec-state-sync] rx closed.")
                    },
                },
                () = self.timer.wait_next().fuse() => {self.sync_state().await;},
            };
        }
    }
}
