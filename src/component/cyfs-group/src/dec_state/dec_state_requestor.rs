// the manager of the DEC's state that synchronized from the group's rpath

use std::sync::Arc;

use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::GroupRPath;
use cyfs_group_lib::GroupRPathStatus;
use cyfs_lib::NONObjectInfo;
use futures::FutureExt;

use crate::{storage::DecStorage, Committee, HotstuffMessage, CHANNEL_CAPACITY};

use super::{CallReplyNotifier, CallReplyWaiter};

enum DecStateRequestorMessage {
    QueryState(String),                                     // sub-path
    VerifiableState(String, BuckyResult<GroupRPathStatus>), // (sub-path, result)
}

struct DecStateRequestorRaw {
    local_device_id: ObjectId,
    tx_dec_state_req_message: async_std::channel::Sender<(DecStateRequestorMessage, ObjectId)>,
    query_state_notifier: CallReplyNotifier<String, BuckyResult<Option<NONObjectInfo>>>,
}

#[derive(Clone)]
pub struct DecStateRequestor(Arc<DecStateRequestorRaw>);

impl DecStateRequestor {
    pub(crate) fn new(
        local_device_id: ObjectId,
        rpath: GroupRPath,
        committee: Committee,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        store: DecStorage,
    ) -> Self {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let notifier = CallReplyNotifier::new();

        let mut runner = DecStateRequestorRunner::new(
            local_device_id,
            rpath,
            committee,
            rx,
            store,
            network_sender,
            non_driver,
            notifier.clone(),
        );

        async_std::task::spawn(async move { runner.run().await });

        Self(Arc::new(DecStateRequestorRaw {
            local_device_id,
            tx_dec_state_req_message: tx,
            query_state_notifier: notifier,
        }))
    }

    pub async fn wait_query_state(
        &self,
        sub_path: String,
    ) -> CallReplyWaiter<BuckyResult<Option<NONObjectInfo>>> {
        self.0.query_state_notifier.prepare(sub_path).await
    }

    pub async fn on_query_state(&self, sub_path: String, remote: ObjectId) {
        if let Err(err) = self
            .0
            .tx_dec_state_req_message
            .send((
                DecStateRequestorMessage::QueryState(sub_path.clone()),
                remote.clone(),
            ))
            .await
        {
            log::warn!("post query state command to processor failed will ignore it, sub_path: {}, rmote: {}, err: {:?}", sub_path, remote, err);
        }
    }

    pub async fn on_verifiable_state(
        &self,
        sub_path: String,
        result: BuckyResult<GroupRPathStatus>,
        remote: ObjectId,
    ) {
        if let Err(err) = self
            .0
            .tx_dec_state_req_message
            .send((
                DecStateRequestorMessage::VerifiableState(sub_path.clone(), result),
                remote,
            ))
            .await
        {
            log::warn!("post verifiable state command to processor failed will ignore it, sub_path: {}, rmote: {}, err: {:?}", sub_path, remote, err);
        }
    }
}

struct DecStateRequestorRunner {
    local_device_id: ObjectId,
    rpath: GroupRPath,
    committee: Committee,
    rx_dec_state_req_message: async_std::channel::Receiver<(DecStateRequestorMessage, ObjectId)>,
    // timer: Timer,
    store: DecStorage,

    network_sender: crate::network::Sender,
    non_driver: crate::network::NONDriverHelper,
    query_state_notifier: CallReplyNotifier<String, BuckyResult<Option<NONObjectInfo>>>,
}

impl DecStateRequestorRunner {
    fn new(
        local_device_id: ObjectId,
        rpath: GroupRPath,
        committee: Committee,
        rx_dec_state_req_message: async_std::channel::Receiver<(
            DecStateRequestorMessage,
            ObjectId,
        )>,
        store: DecStorage,
        network_sender: crate::network::Sender,
        non_driver: crate::network::NONDriverHelper,
        query_state_notifier: CallReplyNotifier<String, BuckyResult<Option<NONObjectInfo>>>,
    ) -> Self {
        Self {
            local_device_id,
            rpath,
            rx_dec_state_req_message,
            // timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            store,
            query_state_notifier,
            network_sender,
            non_driver,
            committee,
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
                    .check_sub_path_value(sub_path.as_str(), &result, &remote)
                    .await
                    .map(|r| r.cloned());

                log::debug!(
                    "handle_verifiable_state sub_path: {}, result: {:?}",
                    sub_path,
                    result
                );
                self.query_state_notifier.reply(&sub_path, result).await
            }
            Err(e) => self.query_state_notifier.reply(&sub_path, Err(e)).await,
        }
    }

    async fn check_sub_path_value<'a>(
        &self,
        sub_path: &str,
        verifiable_status: &'a GroupRPathStatus,
        remote: &ObjectId,
    ) -> BuckyResult<Option<&'a NONObjectInfo>> {
        self.committee
            .verify_block_desc_with_qc(
                &verifiable_status.block_desc,
                &verifiable_status.certificate,
                remote.clone(),
            )
            .await?;

        self.store
            .check_sub_path_value(sub_path, verifiable_status)
            .await
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                message = self.rx_dec_state_req_message.recv().fuse() => match message {
                    Ok((DecStateRequestorMessage::QueryState(sub_path), remote)) => self.handle_query_state(sub_path, remote).await,
                    Ok((DecStateRequestorMessage::VerifiableState(sub_path, result), remote)) => self.handle_verifiable_state(sub_path, result, remote).await,
                    Err(e) => {
                        log::warn!("[dec-state-sync] rx closed, err: {:?}.", e);
                    },
                },
                // () = self.timer.wait_next().fuse() => {self.sync_state().await;},
            };
        }
    }
}
