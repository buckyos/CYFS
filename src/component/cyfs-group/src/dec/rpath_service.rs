use std::sync::Arc;

use cyfs_base::{
    AnyNamedObject, BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId,
    RawConvertTo, RawFrom, RsaCPUObjectSigner, TypelessCoreObject,
};
use cyfs_core::{GroupProposal, GroupRPath};
use cyfs_lib::NONObjectInfo;

use crate::{
    network::NONDriverHelper,
    storage::{GroupShellManager, GroupStorage},
    Committee, Hotstuff, HotstuffMessage, PendingProposalHandler, PendingProposalMgr,
    RPathEventNotifier,
};

struct RPathServiceRaw {
    local_id: ObjectId,
    rpath: GroupRPath,
    network_sender: crate::network::Sender,
    pending_proposal_handle: PendingProposalHandler,
    hotstuff: Hotstuff,
    non_driver: NONDriverHelper,
}

#[derive(Clone)]
pub struct RPathService(Arc<RPathServiceRaw>);

impl RPathService {
    pub(crate) fn start(
        local_id: ObjectId,
        local_device_id: ObjectId,
        rpath: GroupRPath,
        signer: Arc<RsaCPUObjectSigner>,
        event_notifier: RPathEventNotifier,
        network_sender: crate::network::Sender,
        non_driver: NONDriverHelper,
        shell_mgr: GroupShellManager,
        store: GroupStorage,
    ) -> Self {
        let (pending_proposal_handle, pending_proposal_consumer) = PendingProposalMgr::new();
        let committee = Committee::new(
            rpath.group_id().clone(),
            non_driver.clone(),
            shell_mgr.clone(),
            local_device_id,
        );
        let hotstuff = Hotstuff::new(
            local_id,
            local_device_id,
            committee.clone(),
            store,
            signer,
            network_sender.clone(),
            non_driver.clone(),
            shell_mgr,
            pending_proposal_consumer,
            event_notifier,
            rpath.clone(),
        );

        let raw = RPathServiceRaw {
            network_sender,
            pending_proposal_handle,
            local_id,
            rpath,
            hotstuff,
            non_driver,
        };

        Self(Arc::new(raw))
    }

    pub fn rpath(&self) -> &GroupRPath {
        &self.0.rpath
    }

    pub async fn push_proposal(
        &self,
        proposal: GroupProposal,
    ) -> BuckyResult<Option<NONObjectInfo>> {
        let proposal_id = proposal.desc().object_id();

        log::info!("group({:?}) push proposal {}", self.rpath(), proposal_id);

        let object_raw = proposal.to_vec()?;
        let any_obj =
            AnyNamedObject::Core(TypelessCoreObject::clone_from_slice(object_raw.as_slice())?);
        let non_obj = NONObjectInfo::new(proposal_id, object_raw, Some(Arc::new(any_obj)));
        self.0.non_driver.put_object(non_obj).await?;
        self.0.pending_proposal_handle.on_proposal(proposal).await?;

        let waiter = self.0.hotstuff.wait_proposal_result(proposal_id).await;

        waiter.wait().await.map_or(
            Err(BuckyError::new(BuckyErrorCode::Unknown, "unknown")),
            |r| r,
        )
    }

    pub fn select_branch(&self, _block_id: ObjectId, _source: ObjectId) -> BuckyResult<()> {
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
