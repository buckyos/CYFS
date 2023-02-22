// the manager of the DEC's state that synchronized from the group's rpath

use std::sync::Arc;

use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId};
use cyfs_core::{GroupConsensusBlockObject, GroupRPath};
use futures::FutureExt;

use crate::{
    helper::verify_rpath_value, storage::DecStorage, GroupRPathStatus, HotstuffMessage,
    CHANNEL_CAPACITY,
};

use super::{CallReplyNotifier, CallReplyWaiter};

enum DecStateRequestorMessage {
    QueryState(String),                                     // sub-path
    VerifiableState(String, BuckyResult<GroupRPathStatus>), // (sub-path, result)
}

struct DecStateRequestorRaw {
    local_id: ObjectId,
    tx_dec_state_req_message: async_std::channel::Sender<(DecStateRequestorMessage, ObjectId)>,
    query_state_notifier: CallReplyNotifier<String, BuckyResult<ObjectId>>,
}

#[derive(Clone)]
pub struct DecStateRequestor(Arc<DecStateRequestorRaw>);

impl DecStateRequestor {
    pub(crate) fn new(
        local_id: ObjectId,
        rpath: GroupRPath,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        store: DecStorage,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let notifier = CallReplyNotifier::new();

        let mut runner = DecStateRequestorRunner::new(
            local_id,
            rpath,
            rx,
            store,
            network_sender,
            non_driver,
            notifier.clone(),
        );

        async_std::task::spawn(async move { runner.run().await });

        Self(Arc::new(DecStateRequestorRaw {
            local_id,
            tx_dec_state_req_message: tx,
            query_state_notifier: notifier,
        }))
    }

    pub async fn wait_query_state(
        &self,
        sub_path: String,
    ) -> CallReplyWaiter<BuckyResult<ObjectId>> {
        self.0.query_state_notifier.prepare(sub_path).await
    }

    pub async fn on_query_state(&self, sub_path: String, remote: ObjectId) {
        self.0
            .tx_dec_state_req_message
            .send((DecStateRequestorMessage::QueryState(sub_path), remote))
            .await;
    }

    pub async fn on_verifiable_state(
        &self,
        sub_path: String,
        result: BuckyResult<GroupRPathStatus>,
        remote: ObjectId,
    ) {
        self.0
            .tx_dec_state_req_message
            .send((
                DecStateRequestorMessage::VerifiableState(sub_path, result),
                remote,
            ))
            .await;
    }
}

struct DecStateRequestorRunner {
    local_id: ObjectId,
    rpath: GroupRPath,
    rx_dec_state_req_message: async_std::channel::Receiver<(DecStateRequestorMessage, ObjectId)>,
    // timer: Timer,
    store: DecStorage,

    network_sender: crate::network::Sender,
    non_driver: crate::network::NONDriverHelper,
    query_state_notifier: CallReplyNotifier<String, BuckyResult<ObjectId>>,
}

impl DecStateRequestorRunner {
    fn new(
        local_id: ObjectId,
        rpath: GroupRPath,
        rx_dec_state_req_message: async_std::channel::Receiver<(
            DecStateRequestorMessage,
            ObjectId,
        )>,
        store: DecStorage,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        query_state_notifier: CallReplyNotifier<String, BuckyResult<ObjectId>>,
    ) -> Self {
        Self {
            local_id,
            rpath,
            rx_dec_state_req_message,
            // timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            store,
            query_state_notifier,
            network_sender,
            non_driver,
        }
    }

    async fn handle_query_state(&mut self, sub_path: String, remote: ObjectId) {
        let result = self.store.get_by_path(sub_path.as_str()).await;
        self.network_sender
            .post_message(
                HotstuffMessage::VerifiableState(sub_path, result),
                self.rpath.clone(),
                &remote,
            )
            .await;
    }

    async fn handle_verifiable_state(
        &mut self,
        sub_path: String,
        result: BuckyResult<GroupRPathStatus>,
        remote: ObjectId,
    ) {
        match result {
            Ok(result) => {
                let result = self
                    .verify_verifiable_state(sub_path.as_str(), &result, &remote)
                    .await
                    .map(|_| unimplemented!()); // TODO: 搜索目标值

                self.query_state_notifier.reply(&sub_path, result).await
            }
            Err(e) => self.query_state_notifier.reply(&sub_path, Err(e)).await,
        }
    }

    async fn verify_verifiable_state(
        &self,
        sub_path: &str,
        result: &GroupRPathStatus,
        remote: &ObjectId,
    ) -> BuckyResult<()> {
        // let header_block = self
        //     .non_driver
        //     .get_block(&result.block_id, Some(remote))
        //     .await?;
        // let qc_block = self
        //     .non_driver
        //     .get_block(&result.qc_block_id, Some(remote))
        //     .await?;

        let qc = &result.certificate;

        let group = self
            .non_driver
            .get_group(
                self.rpath.group_id(),
                Some(result.block_desc.content().group_chunk_id()),
                Some(&remote),
            )
            .await?;

        if !verify_rpath_value(&result, sub_path, &header_block, qc, &group).await? {
            Err(BuckyError::new(
                BuckyErrorCode::InvalidSignature,
                "verify failed",
            ))
        } else {
            Ok(())
        }
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                message = self.rx_dec_state_req_message.recv().fuse() => match message {
                    Ok((DecStateRequestorMessage::QueryState(sub_path), remote)) => self.handle_query_state(sub_path, remote).await,
                    Ok((DecStateRequestorMessage::VerifiableState(sub_path, result), remote)) => self.handle_verifiable_state(sub_path, result, remote).await,
                    Err(e) => {
                        log::warn!("[dec-state-sync] rx closed.")
                    },
                },
                // () = self.timer.wait_next().fuse() => {self.sync_state().await;},
            };
        }
    }
}
