use std::collections::HashMap;

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_core::GroupProposal;
use futures::FutureExt;

use crate::AsProposal;

pub enum ProposalConsumeMessage {
    Query(Sender<Vec<GroupProposal>>),
    Wait(Sender<()>),
    Remove(Vec<ObjectId>),
}

pub struct PendingProposalMgr {
    rx_product: Receiver<GroupProposal>,
    rx_consume: Receiver<ProposalConsumeMessage>,
    tx_proposal_waker: Option<Sender<()>>,
    network_sender: crate::network::Sender,

    // TODO: 需要设计一个结构便于按时间或数量拆分
    buffer: HashMap<ObjectId, GroupProposal>,
}

impl PendingProposalMgr {
    pub fn spawn(
        rx_product: Receiver<GroupProposal>,
        rx_consume: Receiver<ProposalConsumeMessage>,
        network_sender: crate::network::Sender,
    ) {
        async_std::task::spawn(async move {
            Self {
                rx_product,
                rx_consume,
                buffer: HashMap::new(),
                network_sender,
                tx_proposal_waker: None,
            }
            .run()
            .await
        });
    }

    pub async fn query_proposals(
        tx_consume: &Sender<ProposalConsumeMessage>,
    ) -> BuckyResult<Vec<GroupProposal>> {
        let (sender, receiver) = async_std::channel::bounded(1);
        tx_consume
            .send(ProposalConsumeMessage::Query(sender))
            .await
            .map_err(|e| {
                log::error!("[pending_proposal_mgr] send message(query) faield: {}", e);
                BuckyError::new(BuckyErrorCode::ErrorState, "channel closed")
            })?;

        receiver.recv().await.map_err(|e| {
            log::error!("[pending_proposal_mgr] recv message(query) failed: {}", e);
            BuckyError::new(BuckyErrorCode::ErrorState, "channel closed")
        })
    }

    pub async fn remove_proposals(
        tx_consume: &Sender<ProposalConsumeMessage>,
        proposal_ids: Vec<ObjectId>,
    ) -> BuckyResult<()> {
        tx_consume
            .send(ProposalConsumeMessage::Remove(proposal_ids))
            .await
            .map_err(|e| {
                log::error!("[pending_proposal_mgr] send message(remove) faield: {}", e);
                BuckyError::new(BuckyErrorCode::ErrorState, "channel closed")
            })
    }

    async fn query_proposals_impl(&mut self) -> Vec<GroupProposal> {
        self.buffer.iter().map(|(_, p)| p.clone()).collect()
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                proposal = self.rx_product.recv().fuse() => {
                    if let Ok(proposal) = proposal {
                        self.buffer.insert(proposal.id(), proposal);
                        if let Some(waker) = self.tx_proposal_waker.take() {
                            waker.send(()).await;
                        }
                    }
                },
                message = self.rx_consume.recv().fuse() => {
                    if let Ok(message) = message {
                       match message {
                            ProposalConsumeMessage::Query(sender) => {
                                let proposals = self.query_proposals_impl().await;
                                sender.send(proposals).await;
                            },
                            ProposalConsumeMessage::Remove(proposal_ids) => {
                                for id in &proposal_ids {
                                    self.buffer.remove(id);
                                }
                            },
                            ProposalConsumeMessage::Wait(tx_waker) => {
                                if self.buffer.len() > 0 {
                                    tx_waker.send(()).await;
                                } else {
                                    self.tx_proposal_waker = Some(tx_waker)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
