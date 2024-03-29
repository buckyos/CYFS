// the manager of the DEC's state that synchronized from the group's rpath

use std::{collections::HashSet, sync::Arc};

use cyfs_base::{BuckyResult, Group, NamedObject, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath, HotstuffBlockQC};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{
    network::NONDriverHelper,
    storage::{DecStorage, DecStorageCache, GroupShellManager},
    Committee, CHANNEL_CAPACITY,
};

use super::{CallReplyNotifier, CallReplyWaiter};

enum DecStateSynchronizerMessage {
    ProposalResult(
        ObjectId,
        BuckyResult<(Option<NONObjectInfo>, GroupConsensusBlock, HotstuffBlockQC)>,
    ),
    StateChange(GroupConsensusBlock, HotstuffBlockQC),
    DelaySync(Option<(ObjectId, Option<NONObjectInfo>)>), // (proposal-id, Ok(result))
}

struct DecStateSynchronizerRaw {
    local_device_id: ObjectId,
    tx_dec_state_sync_message: async_std::channel::Sender<(DecStateSynchronizerMessage, ObjectId)>,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

#[derive(Clone)]
pub struct DecStateSynchronizer(Arc<DecStateSynchronizerRaw>);

impl DecStateSynchronizer {
    pub(crate) fn new(
        local_device_id: ObjectId,
        rpath: GroupRPath,
        committee: Committee,
        non_driver: crate::network::NONDriverHelper,
        shell_mgr: GroupShellManager,
        store: DecStorage,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let notifier = CallReplyNotifier::new();

        let mut runner = DecStateSynchronizerRunner::new(
            local_device_id,
            rpath,
            committee,
            tx.clone(),
            rx,
            store,
            non_driver,
            shell_mgr,
            notifier.clone(),
        );

        async_std::task::spawn(async move { runner.run().await });

        Self(Arc::new(DecStateSynchronizerRaw {
            local_device_id,
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
        result: BuckyResult<(Option<NONObjectInfo>, GroupConsensusBlock, HotstuffBlockQC)>,
        remote: ObjectId,
    ) {
        if let Err(err) = self
            .0
            .tx_dec_state_sync_message
            .send((
                DecStateSynchronizerMessage::ProposalResult(proposal_id.clone(), result),
                remote.clone(),
            ))
            .await
        {
            log::warn!("post proposal complete notification failed, proposal_id: {}, remote: {}, err: {:?}.", proposal_id, remote, err);
        }
    }

    pub async fn on_state_change(
        &self,
        header_block: GroupConsensusBlock,
        qc: HotstuffBlockQC,
        remote: ObjectId,
    ) {
        let new_header_id = header_block.block_id().clone();
        if let Err(err) = self
            .0
            .tx_dec_state_sync_message
            .send((
                DecStateSynchronizerMessage::StateChange(header_block, qc),
                remote,
            ))
            .await
        {
            log::warn!("post block state change notification failed, new-header: {}, remote: {}, err: {:?}.", new_header_id, remote, err);
        }
    }
}

struct DecStateSynchronizerCache {
    state_cache: DecStorageCache,
    group_shell_id: ObjectId,
    group: Group,
}

struct UpdateNotifyInfo {
    header_block: GroupConsensusBlock,
    qc: HotstuffBlockQC,
    remotes: HashSet<ObjectId>,
    group_shell_id: ObjectId,
    group: Group,
}

struct DecStateSynchronizerRunner {
    local_device_id: ObjectId,
    rpath: GroupRPath,
    committee: Committee,
    tx_dec_state_sync_message: async_std::channel::Sender<(DecStateSynchronizerMessage, ObjectId)>,
    rx_dec_state_sync_message:
        async_std::channel::Receiver<(DecStateSynchronizerMessage, ObjectId)>,
    // timer: Timer,
    store: DecStorage,
    state_cache: Option<DecStateSynchronizerCache>,
    update_notifies: Option<UpdateNotifyInfo>,

    non_driver: NONDriverHelper,
    shell_mgr: GroupShellManager,
    proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
}

impl DecStateSynchronizerRunner {
    fn new(
        local_device_id: ObjectId,
        rpath: GroupRPath,
        committee: Committee,
        tx_dec_state_sync_message: async_std::channel::Sender<(
            DecStateSynchronizerMessage,
            ObjectId,
        )>,
        rx_dec_state_sync_message: async_std::channel::Receiver<(
            DecStateSynchronizerMessage,
            ObjectId,
        )>,
        store: DecStorage,
        non_driver: NONDriverHelper,
        shell_mgr: GroupShellManager,
        proposal_result_notifier: CallReplyNotifier<ObjectId, BuckyResult<Option<NONObjectInfo>>>,
    ) -> Self {
        Self {
            local_device_id,
            rpath,
            tx_dec_state_sync_message,
            rx_dec_state_sync_message,
            // timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            store,
            state_cache: None,
            update_notifies: None,
            non_driver,
            proposal_result_notifier,
            committee,
            shell_mgr,
        }
    }

    async fn handle_proposal_complete(
        &mut self,
        proposal_id: ObjectId,
        result: BuckyResult<(Option<NONObjectInfo>, GroupConsensusBlock, HotstuffBlockQC)>,
        remote: ObjectId,
    ) {
        match result {
            Ok((result, header_block, qc)) => {
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
                    .push_update_notify(header_block, qc, remote)
                    .await
                    .is_err()
                {
                    return;
                }

                if let Err(err) = self
                    .tx_dec_state_sync_message
                    .send((
                        DecStateSynchronizerMessage::DelaySync(Some((proposal_id, result))),
                        remote,
                    ))
                    .await
                {
                    log::warn!("post delay sync state message for new proposal complete failed, proposal: {} err: {:?}.", proposal_id, err);
                }
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
        qc: HotstuffBlockQC,
        remote: ObjectId,
    ) {
        let header_id = header_block.block_id().clone();
        if self
            .push_update_notify(header_block, qc, remote)
            .await
            .is_ok()
        {
            if let Err(err) = self
                .tx_dec_state_sync_message
                .send((DecStateSynchronizerMessage::DelaySync(None), remote))
                .await
            {
                log::warn!("post delay sync state message for header changed failed, new-header: {} err: {:?}.", header_id, err);
            }
        }
    }

    async fn sync_state(
        &mut self,
        proposal_result: Option<(ObjectId, Option<NONObjectInfo>)>,
        _remote: ObjectId,
    ) {
        let result = match self.update_notifies.as_ref() {
            Some(notify_info) => {
                let mut err = None;
                for remote in notify_info.remotes.iter() {
                    match self
                        .store
                        .sync(&notify_info.header_block, &notify_info.qc, remote.clone())
                        .await
                    {
                        Ok(_) => {
                            err = None;
                            self.state_cache = Some(DecStateSynchronizerCache {
                                state_cache: DecStorageCache {
                                    state: notify_info.header_block.result_state_id().clone(),
                                    header_block: notify_info.header_block.clone(),
                                    qc: notify_info.qc.clone(),
                                },
                                group_shell_id: notify_info.group_shell_id,
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
                let group_shell_id = state_cache.header_block.group_shell_id().clone();
                let group = self
                    .shell_mgr
                    .get_group(self.rpath.group_id(), Some(&group_shell_id), None)
                    .await;
                if let Ok(group) = group {
                    self.state_cache = Some(DecStateSynchronizerCache {
                        state_cache,
                        group_shell_id,
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
        qc: HotstuffBlockQC,
        remote: ObjectId,
    ) -> BuckyResult<()> {
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

        if header_block.check()
            && self
                .committee
                .verify_block_desc_with_qc(header_block.named_object().desc(), &qc, remote)
                .await
                .is_ok()
        {
            let group = self
                .shell_mgr
                .get_group(
                    self.rpath.group_id(),
                    Some(header_block.group_shell_id()),
                    None,
                )
                .await?;
            let group_shell_id = header_block.group_shell_id().clone();

            self.update_notifies = Some(UpdateNotifyInfo {
                header_block: header_block,
                qc: qc,
                remotes: HashSet::from([remote]),
                group_shell_id,
                group,
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
                        log::warn!("[dec-state-sync] rx closed, err: {:?}.", e);
                    },
                },
                // () = self.timer.wait_next().fuse() => {self.sync_state().await;},
            };
        }
    }
}
