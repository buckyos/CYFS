// notify the members when the state of rpath changed

use std::collections::HashSet;

use cyfs_base::{
    BuckyError, BuckyResult, GroupMemberScope, NamedObject, ObjectDesc, ObjectId, OwnerObjectDesc,
    RawDecode,
};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupProposal, GroupRPath};
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{
    helper::Timer, HotstuffMessage, CHANNEL_CAPACITY, STATE_NOTIFY_CYCLE, SYNCHRONIZER_TIMEOUT,
};

enum StatePushMessage {
    ProposalResult(GroupProposal, BuckyError),
    BlockCommit(GroupConsensusBlock, GroupConsensusBlock),
    LastStateRequest(ObjectId),
}

pub struct StatePusher {
    local_id: ObjectId,
    tx_notifier: async_std::channel::Sender<StatePushMessage>,
}

impl StatePusher {
    pub fn new(
        local_id: ObjectId,
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        non_driver: crate::network::NonDriver,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);

        let mut runner = StateChanggeRunner::new(local_id, network_sender, rpath, non_driver, rx);

        async_std::task::spawn(async move { runner.run().await });

        Self {
            local_id,
            tx_notifier: tx,
        }
    }

    pub async fn notify_proposal_err(&self, proposal: GroupProposal, err: BuckyError) {
        self.tx_notifier
            .send(StatePushMessage::ProposalResult(proposal, err))
            .await;
    }

    pub async fn notify_block_commit(
        &self,
        block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
    ) {
        let block_id = block.named_object().desc().object_id();
        if qc_block.height() != block.height() + 1
            || qc_block.qc().as_ref().expect("qc should not empty").round != block.round()
            || qc_block.round() <= block.round()
            || qc_block.prev_block_id().unwrap() != &block_id
        {
            log::error!(
                "the qc-block({}) should be next block({})",
                qc_block.named_object().desc().object_id(),
                block_id
            );
            return;
        }

        if block.owner() != &self.local_id {
            return;
        }

        self.tx_notifier
            .send(StatePushMessage::BlockCommit(block, qc_block))
            .await;
    }

    pub async fn request_last_state(&self, remote: ObjectId) {
        self.tx_notifier
            .send(StatePushMessage::LastStateRequest(remote))
            .await;
    }
}

struct HeaderBlockNotifyProgress {
    header_block: GroupConsensusBlock,
    qc_block: GroupConsensusBlock,
    group_chunk_id: ObjectId,
    members: Vec<ObjectId>,
    total_notify_times: usize,
    cur_block_notify_times: usize,
}

struct StateChanggeRunner {
    local_id: ObjectId,
    network_sender: crate::network::Sender,
    rpath: GroupRPath,
    non_driver: crate::network::NonDriver,
    rx_notifier: async_std::channel::Receiver<StatePushMessage>,
    timer: Timer,

    request_last_state_remotes: HashSet<ObjectId>,
    notify_progress: Option<HeaderBlockNotifyProgress>,
}

impl StateChanggeRunner {
    fn new(
        local_id: ObjectId,
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        non_driver: crate::network::NonDriver,
        rx_notifier: async_std::channel::Receiver<StatePushMessage>,
    ) -> Self {
        Self {
            network_sender,
            rpath,
            non_driver,
            rx_notifier,
            timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            notify_progress: None,
            local_id,
            request_last_state_remotes: HashSet::new(),
        }
    }

    pub async fn notify_proposal_err(&self, proposal: GroupProposal, err: BuckyError) {
        // notify to the proposer
        let proposal_id = proposal.desc().object_id();
        match proposal.desc().owner() {
            Some(proposer) => {
                let network_sender = self.network_sender.clone();
                let proposer = proposer.clone();
                let rpath = self.rpath.clone();

                network_sender
                    .post_message(
                        HotstuffMessage::ProposalResult(proposal_id, Err(err)),
                        rpath.clone(),
                        &proposer,
                    )
                    .await
            }
            None => log::warn!("proposal({}) without owner", proposal_id),
        }
    }

    pub async fn notify_proposal_result_for_block(
        &self,
        block: &GroupConsensusBlock,
        qc_block: &GroupConsensusBlock,
    ) {
        let network_sender = self.network_sender.clone();
        let rpath = self.rpath.clone();
        let non_driver = self.non_driver.clone();
        let proposal_exe_infos = block.proposals().clone();

        let proposals = futures::future::join_all(
            proposal_exe_infos
                .iter()
                .map(|proposal| non_driver.get_proposal(&proposal.proposal, None)),
        )
        .await;

        for i in 0..proposal_exe_infos.len() {
            let proposal = proposals.get(i).unwrap();
            if proposal.is_err() {
                continue;
            }
            let proposal = proposal.as_ref().unwrap();
            let proposer = proposal.desc().owner();
            if proposer.is_none() {
                continue;
            }

            let proposer = proposer.as_ref().unwrap();
            let exe_info = proposal_exe_infos.get(i).unwrap();

            let receipt = match exe_info.receipt.as_ref() {
                Some(receipt) => match NONObjectInfo::raw_decode(receipt.as_slice()) {
                    Ok((obj, _)) => Some(obj),
                    _ => continue,
                },
                None => None,
            };

            network_sender
                .post_message(
                    HotstuffMessage::ProposalResult(
                        exe_info.proposal,
                        Ok((receipt, block.clone(), qc_block.clone())),
                    ),
                    rpath.clone(),
                    &proposer,
                )
                .await
        }
    }

    async fn update_commit_block(
        &mut self,
        block: GroupConsensusBlock,
        qc_block: GroupConsensusBlock,
    ) {
        match self.notify_progress.as_mut() {
            Some(progress) => {
                if progress.header_block.height() >= block.height() {
                    return;
                }

                if block.group_chunk_id() != progress.header_block.group_chunk_id() {
                    let group = self
                        .non_driver
                        .get_group(block.r_path().group_id(), Some(block.group_chunk_id()))
                        .await;
                    if group.is_err() {
                        return;
                    }
                    progress.members = group
                        .unwrap()
                        .select_members_with_distance(&self.local_id, GroupMemberScope::All)
                        .into_iter()
                        .map(|id| id.clone())
                        .collect();
                }

                progress.group_chunk_id = block.group_chunk_id().clone();
                progress.total_notify_times += progress.cur_block_notify_times;
                progress.cur_block_notify_times = 0;
                progress.header_block = block;
                progress.qc_block = qc_block;
            }
            None => {
                let group = self
                    .non_driver
                    .get_group(block.r_path().group_id(), Some(block.group_chunk_id()))
                    .await;
                if group.is_err() {
                    return;
                }

                let members: Vec<ObjectId> = group
                    .unwrap()
                    .select_members_with_distance(&self.local_id, GroupMemberScope::All)
                    .into_iter()
                    .map(|id| id.clone())
                    .collect();
                let total_notify_times = match members.iter().position(|id| id == &self.local_id) {
                    Some(pos) => pos,
                    None => return,
                };

                let group_chunk_id = block.group_chunk_id().clone();

                self.notify_progress = Some(HeaderBlockNotifyProgress {
                    header_block: block,
                    qc_block,
                    group_chunk_id,
                    members,
                    total_notify_times,
                    cur_block_notify_times: 0,
                });
            }
        }
    }

    fn request_last_state(&mut self, remote: ObjectId) {
        self.request_last_state_remotes.insert(remote);
    }

    async fn try_notify_block_commit(&mut self) {
        match self.notify_progress.as_mut() {
            Some(progress) if progress.cur_block_notify_times < progress.members.len() => {
                let notify_count = (progress.members.len() * SYNCHRONIZER_TIMEOUT as usize
                    / STATE_NOTIFY_CYCLE as usize)
                    .max(1)
                    .min(progress.members.len() - progress.cur_block_notify_times);

                progress.cur_block_notify_times += notify_count;

                let mut notify_targets = HashSet::new();
                std::mem::swap(&mut self.request_last_state_remotes, &mut notify_targets);

                let start_pos = progress.cur_block_notify_times % progress.members.len();
                let notify_targets_1 = &progress.members.as_slice()
                    [start_pos..progress.members.len().min(start_pos + notify_count)];

                notify_targets_1.iter().for_each(|remote| {
                    notify_targets.insert(remote.clone());
                });

                if notify_targets_1.len() < notify_count {
                    let notify_targets_2 =
                        &progress.members.as_slice()[0..notify_count - notify_targets.len()];

                    notify_targets_2.iter().for_each(|remote| {
                        notify_targets.insert(remote.clone());
                    });
                }

                let msg = HotstuffMessage::StateChangeNotify(
                    progress.header_block.clone(),
                    progress.qc_block.clone(),
                );

                let notify_targets: Vec<ObjectId> = notify_targets.into_iter().collect();
                self.network_sender
                    .broadcast(msg.clone(), self.rpath.clone(), notify_targets.as_slice())
                    .await;
            }
            _ => return,
        }
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                message = self.rx_notifier.recv().fuse() => match message {
                    Ok(StatePushMessage::ProposalResult(proposal, err)) => self.notify_proposal_err(proposal, err).await,
                    Ok(StatePushMessage::BlockCommit(block, qc_block)) => {
                        self.notify_proposal_result_for_block(&block, &qc_block).await;
                        self.update_commit_block(block, qc_block).await;
                    },
                    Ok(StatePushMessage::LastStateRequest(remote)) => {
                        self.request_last_state(remote);
                    },
                    Err(e) => {
                        log::warn!("[change-notifier] rx_notifier closed.")
                    },
                },
                () = self.timer.wait_next().fuse() => self.try_notify_block_commit().await,
            };
        }
    }
}
