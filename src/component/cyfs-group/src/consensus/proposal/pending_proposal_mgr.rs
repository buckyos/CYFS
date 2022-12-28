use std::collections::HashMap;

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_core::GroupProposal;
use futures::FutureExt;

use crate::{AsProposal, BlockBuilder, GroupBlockBuilder, ASYNC_TIMEOUT};

pub enum ProposalConsumeMessage {
    BuildBlock(GroupBlockBuilder),
    Remove(Vec<ObjectId>),
}

pub struct PendingProposalMgr {
    rx_product: Receiver<GroupProposal>,
    rx_consume: Receiver<ProposalConsumeMessage>,
    network_sender: crate::network::Sender,

    // TODO: 需要设计一个结构便于按时间或数量拆分
    // TODO: 要把修改group的提案单独排序处理
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
            }
            .run()
            .await
        });
    }

    pub async fn build_block(
        tx_consume: &Sender<ProposalConsumeMessage>,
        builder: GroupBlockBuilder,
    ) -> BuckyResult<()> {
        tx_consume
            .send(ProposalConsumeMessage::BuildBlock(builder))
            .await
            .map_err(|e| {
                log::error!("[pending_proposal_mgr] send message(query) faield: {}", e);
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

    async fn build_block_impl(&mut self, builder: GroupBlockBuilder) -> BuckyResult<()> {
        let proposals = self.buffer.drain().map(|(_, p)| p).collect();

        let block = builder.build(proposals).await?;
        if block.is_none() {
            return Ok(());
        }

        /**
         * TODO:
         * 1. broadcast
         */
        Ok(())
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                proposal = self.rx_product.recv().fuse() => {
                    if let Ok(proposal) = proposal {
                        self.buffer.insert(proposal.id(), proposal);
                    }
                },
                message = self.rx_consume.recv().fuse() => {
                    if let Ok(message) = message {
                       match message {
                            ProposalConsumeMessage::BuildBlock(builder) => {
                                let r = self.build_block_impl(builder).await;
                                notifier.send(r).await;
                            },
                            ProposalConsumeMessage::Remove(proposal_ids) => {
                                for id in &proposal_ids {
                                    self.buffer.remove(id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
