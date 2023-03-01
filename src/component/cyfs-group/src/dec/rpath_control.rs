use std::sync::Arc;

use cyfs_base::{BuckyResult, ObjectId, RsaCPUObjectSigner};
use cyfs_core::{GroupProposal, GroupRPath};
use cyfs_lib::RPathDelegate;

use crate::{
    network::NONDriverHelper, storage::GroupStorage, Committee, Hotstuff, HotstuffMessage,
    PendingProposalHandler, PendingProposalMgr,
};

struct RPathControlRaw {
    local_id: ObjectId,
    rpath: GroupRPath,
    network_sender: crate::network::Sender,
    pending_proposal_handle: PendingProposalHandler,
    hotstuff: Hotstuff,
}

#[derive(Clone)]
pub struct RPathControl(Arc<RPathControlRaw>);

impl RPathControl {
    pub(crate) async fn load(
        local_id: ObjectId,
        local_device_id: ObjectId,
        rpath: GroupRPath,
        signer: Arc<RsaCPUObjectSigner>,
        delegate: Arc<Box<dyn RPathDelegate>>,
        network_sender: crate::network::Sender,
        non_driver: NONDriverHelper,
        store: GroupStorage,
    ) -> BuckyResult<Self> {
        let (pending_proposal_handle, pending_proposal_consumer) = PendingProposalMgr::new();
        let committee = Committee::new(
            rpath.group_id().clone(),
            non_driver.clone(),
            local_device_id,
        );
        let hotstuff = Hotstuff::new(
            local_id,
            local_device_id,
            committee.clone(),
            store,
            signer,
            network_sender.clone(),
            non_driver,
            pending_proposal_consumer,
            delegate,
            rpath.clone(),
        );

        let raw = RPathControlRaw {
            network_sender,
            pending_proposal_handle,
            local_id,
            rpath,
            hotstuff,
        };

        Ok(Self(Arc::new(raw)))
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.0.rpath
    }

    pub async fn push_proposal(&self, proposal: GroupProposal) -> BuckyResult<()> {
        self.0.pending_proposal_handle.on_proposal(proposal).await
    }

    pub fn select_branch(&self, block_id: ObjectId, source: ObjectId) -> BuckyResult<()> {
        unimplemented!()
    }

    pub(crate) async fn on_message(&self, msg: HotstuffMessage, remote: ObjectId) {
        match msg {
            HotstuffMessage::Block(block) => self.0.hotstuff.on_block(block, remote).await,
            HotstuffMessage::BlockVote(vote) => self.0.hotstuff.on_block_vote(vote, remote).await,
            HotstuffMessage::TimeoutVote(vote) => {
                self.0.hotstuff.on_timeout_vote(vote, remote).await
            }
            HotstuffMessage::Timeout(tc) => self.0.hotstuff.on_timeout(tc, remote).await,
            HotstuffMessage::SyncRequest(min_bound, max_bound) => {
                self.0
                    .hotstuff
                    .on_sync_request(min_bound, max_bound, remote)
                    .await
            }
            HotstuffMessage::LastStateRequest => self.0.hotstuff.request_last_state(remote).await,
            HotstuffMessage::StateChangeNotify(_, _) => unreachable!(),
            HotstuffMessage::ProposalResult(_, _) => unreachable!(),
            HotstuffMessage::QueryState(sub_path) => {
                self.0.hotstuff.on_query_state(sub_path, remote).await
            }
            HotstuffMessage::VerifiableState(_, _) => unreachable!(),
        }
    }
}
