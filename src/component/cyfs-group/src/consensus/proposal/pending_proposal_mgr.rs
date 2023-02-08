use std::collections::HashMap;

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId};
use cyfs_core::GroupProposal;
use futures::FutureExt;

use crate::CHANNEL_CAPACITY;

pub enum ProposalConsumeMessage {
    Query(Sender<Vec<GroupProposal>>),
    Wait(Sender<()>),
    Remove(Vec<ObjectId>),
}

pub struct PendingProposalMgr {
    rx_product: Receiver<GroupProposal>,
    rx_consume: Receiver<ProposalConsumeMessage>,
    tx_proposal_waker: Option<Sender<()>>,

    // TODO: 需要设计一个结构便于按时间或数量拆分
    buffer: HashMap<ObjectId, GroupProposal>,
}

impl PendingProposalMgr {
    pub fn new() -> (PendingProposalHandler, PendingProposalConsumer) {
        let (tx_product, rx_product) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let (tx_consume, rx_consume) = async_std::channel::bounded(CHANNEL_CAPACITY);

        async_std::task::spawn(async move {
            PendingProposalMgrRunner {
                rx_product,
                rx_consume,
                buffer: HashMap::new(),
                tx_proposal_waker: None,
            }
            .run()
            .await
        });

        (
            PendingProposalHandler { tx_product },
            PendingProposalConsumer { tx_consume },
        )
    }
}

pub struct PendingProposalHandler {
    tx_product: Sender<GroupProposal>,
}

impl PendingProposalHandler {
    pub async fn on_proposal(&self, proposal: GroupProposal) -> BuckyResult<()> {
        self.tx_product.send(proposal).await.map_err(|e| {
            log::error!(
                "[pending_proposal_mgr] send message(on_proposal) faield: {}",
                e
            );
            BuckyError::new(BuckyErrorCode::ErrorState, "channel closed")
        })
    }
}

pub struct PendingProposalConsumer {
    tx_consume: Sender<ProposalConsumeMessage>,
}

impl PendingProposalConsumer {
    pub async fn query_proposals(&self) -> BuckyResult<Vec<GroupProposal>> {
        let (sender, receiver) = async_std::channel::bounded(1);
        self.tx_consume
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

    pub async fn remove_proposals(&self, proposal_ids: Vec<ObjectId>) -> BuckyResult<()> {
        self.tx_consume
            .send(ProposalConsumeMessage::Remove(proposal_ids))
            .await
            .map_err(|e| {
                log::error!("[pending_proposal_mgr] send message(remove) faield: {}", e);
                BuckyError::new(BuckyErrorCode::ErrorState, "channel closed")
            })
    }
}

struct PendingProposalMgrRunner {
    rx_product: Receiver<GroupProposal>,
    rx_consume: Receiver<ProposalConsumeMessage>,
    tx_proposal_waker: Option<Sender<()>>,

    // TODO: 需要设计一个结构便于按时间或数量拆分
    buffer: HashMap<ObjectId, GroupProposal>,
}

impl PendingProposalMgrRunner {
    async fn handle_query_proposals(&mut self) -> Vec<GroupProposal> {
        self.buffer.iter().map(|(_, p)| p.clone()).collect()
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                proposal = self.rx_product.recv().fuse() => {
                    if let Ok(proposal) = proposal {
                        self.buffer.insert(proposal.desc().object_id(), proposal);
                        if let Some(waker) = self.tx_proposal_waker.take() {
                            waker.send(()).await;
                        }
                    }
                },
                message = self.rx_consume.recv().fuse() => {
                    if let Ok(message) = message {
                       match message {
                            ProposalConsumeMessage::Query(sender) => {
                                let proposals = self.handle_query_proposals().await;
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