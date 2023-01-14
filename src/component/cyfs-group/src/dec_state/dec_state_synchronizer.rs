// the manager of the DEC's state that synchronized from the group's rpath

use std::{collections::HashSet, sync::Arc};

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
    CHANNEL_CAPACITY,
};

use super::{CallReplyNotifier, CallReplyWaiter};

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
    DelaySync(Option<(ObjectId, Option<NONObjectInfo>)>), // (proposal-id, Ok(result))
}

struct DecStateSynchronizerRaw {
    local_id: ObjectId,
    tx_dec_state_sync_message: async_std::channel::Sender<(DecStateSynchronizerMessage, ObjectId)>,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

#[derive(Clone)]
pub struct DecStateSynchronizer(Arc<DecStateSynchronizerRaw>);

impl DecStateSynchronizer {
    pub fn new(
        local_id: ObjectId,
        rpath: GroupRPath,
        non_driver: crate::network::NonDriver,
        store: DecStorage,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let notifier = CallReplyNotifier::new();

        let mut runner = DecStateSynchronizerRunner::new(
            local_id,
            rpath,
            tx.clone(),
            rx,
            store,
            non_driver,
            notifier.clone(),
        );

        async_std::task::spawn(async move { runner.run().await });

        Self(Arc::new(DecStateSynchronizerRaw {
            local_id,
            tx_dec_state_sync_message: tx,
            proposal_result_notifier: notifier,
        }))
    }

    pub async fn wait_proposal_result(
        &self,
        proposal_id: ObjectId,
    ) -> CallReplyWaiter<BuckyResult<Option<NONObjectInfo>>> {
        self.0.proposal_result_notifier.prepare(proposal_id).await
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
        self.0
            .tx_dec_state_sync_message
            .send((
                DecStateSynchronizerMessage::ProposalResult(proposal_id, result),
                remote,
            ))
            .await;
    }

    pub async fn on_state_change(
        &self,
        header_block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
        remote: ObjectId,
    ) {
        self.0
            .tx_dec_state_sync_message
            .send((
                DecStateSynchronizerMessage::StateChange(header_block, qc_block),
                remote,
            ))
            .await;
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
    tx_dec_state_sync_message: async_std::channel::Sender<(DecStateSynchronizerMessage, ObjectId)>,
    rx_dec_state_sync_message:
        async_std::channel::Receiver<(DecStateSynchronizerMessage, ObjectId)>,
    // timer: Timer,
    store: DecStorage,
    state_cache: Option<DecStateSynchronizerCache>,
    update_notifies: Option<UpdateNotifyInfo>,

    non_driver: NonDriver,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

impl DecStateSynchronizerRunner {
    fn new(
        local_id: ObjectId,
        rpath: GroupRPath,
        tx_dec_state_sync_message: async_std::channel::Sender<(
            DecStateSynchronizerMessage,
            ObjectId,
        )>,
        rx_dec_state_sync_message: async_std::channel::Receiver<(
            DecStateSynchronizerMessage,
            ObjectId,
        )>,
        store: DecStorage,
        non_driver: NonDriver,
        proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
    ) -> Self {
        Self {
            local_id,
            rpath,
            tx_dec_state_sync_message,
            rx_dec_state_sync_message,
            // timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            store,
            state_cache: None,
            update_notifies: None,
            non_driver,
            proposal_result_notifier,
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
        match result {
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

                self.tx_dec_state_sync_message
                    .send((
                        DecStateSynchronizerMessage::DelaySync(Some((proposal_id, result))),
                        remote,
                    ))
                    .await;
            }
            Err(e) => {
                // notify the app
                self.proposal_result_notifier
                    .reply(&proposal_id, Err(e))
                    .await;
            }
        };
    }

    async fn handle_state_change(
        &mut self,
        header_block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
        remote: ObjectId,
    ) {
        if self
            .push_update_notify(header_block, qc_block, remote)
            .await
            .is_ok()
        {
            self.tx_dec_state_sync_message
                .send((DecStateSynchronizerMessage::DelaySync(None), remote))
                .await;
        }
    }

    async fn sync_state(
        &mut self,
        proposal_result: Option<(ObjectId, Option<NONObjectInfo>)>,
        remote: ObjectId,
    ) {
        let result = match self.update_notifies.as_ref() {
            Some(notify_info) => {
                let mut err = None;
                for remote in notify_info.remotes.iter() {
                    match self
                        .store
                        .sync(
                            &notify_info.header_block,
                            &notify_info.qc_block,
                            remote.clone(),
                        )
                        .await
                    {
                        Ok(_) => {
                            err = None;
                            self.state_cache = Some(DecStateSynchronizerCache {
                                state_cache: DecStorageCache {
                                    state: notify_info.header_block.result_state_id().clone(),
                                    header_block: notify_info.header_block.clone(),
                                    qc_block: notify_info.qc_block.clone(),
                                },
                                group_chunk_id: notify_info.group_chunk_id,
                                group: notify_info.group.clone(),
                            });
                            self.update_notifies = None;
                            break;
                        }
                        Err(e) => {
                            err = err.or(Some(e));
                        }
                    }
                }
                err.map_or(Ok(()), |e| Err(e))
            }
            None => Ok(()),
        };

        if let Some((proposal_id, proposal_result)) = proposal_result {
            let proposal_result = result.map(|_| proposal_result);
            // notify app dec
            self.proposal_result_notifier
                .reply(&proposal_id, proposal_result)
                .await;
        }
    }

    async fn check_cache(&mut self) -> &Option<DecStateSynchronizerCache> {
        if self.state_cache.is_none() {
            let state_cache = self.store.cur_state().await;
            if let Some(state_cache) = state_cache {
                let group_chunk_id = state_cache.header_block.group_chunk_id().clone();
                let group = self
                    .non_driver
                    .get_group(self.rpath.group_id(), Some(&group_chunk_id), None)
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
                    .get_group(
                        self.rpath.group_id(),
                        Some(header_block.group_chunk_id()),
                        Some(&remote),
                    )
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
                    Ok((DecStateSynchronizerMessage::DelaySync(proposal_result), remote)) => self.sync_state(proposal_result, remote).await,
                    Err(e) => {
                        log::warn!("[dec-state-sync] rx closed.")
                    },
                },
                // () = self.timer.wait_next().fuse() => {self.sync_state().await;},
            };
        }
    }
}
