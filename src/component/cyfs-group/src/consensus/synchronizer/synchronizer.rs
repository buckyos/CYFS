use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
    vec,
};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject, GroupRPath};
use protobuf::well_known_types::Duration;

use crate::{
    consensus::timer::Timer, storage::Storage, HotstuffMessage, SyncBound, CHANNEL_CAPACITY,
    SYNCHRONIZER_TIMEOUT, SYNCHRONIZER_TRY_TIMES,
};

impl SyncBound {
    fn value(&self) -> u64 {
        match self {
            Self::Height(h) => h,
            Self::Round(r) => r,
        }
    }

    fn add(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h + value),
            Self::Round(r) => Self::Round(*r + value),
        }
    }

    fn sub(&self, value: u64) -> Self {
        match self {
            Self::Height(h) => Self::Height(*h - value),
            Self::Round(r) => Self::Round(*r - value),
        }
    }
}

impl Ord for SyncBound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self {
            Self::Height(height) => match other {
                Self::Height(other_height) => height.cmp(other_height),
                Self::Round(other_round) => {
                    if height >= other_round {
                        std::cmp::Ordering::Greater
                    } else {
                        std::cmp::Ordering::Less
                    }
                }
            },
            Self::Round(round) => match other {
                Self::Round(other_round) => round.cmp(other_round),
                Self::Height(other_height) => {
                    if other_height >= round {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Greater
                    }
                }
            },
        }
    }
}

enum SynchronizerMessage {
    Sync(u64, SyncBound, ObjectId), // ([min-height, max-bound], remote)
    PushBlock(u64, GroupConsensusBlock, ObjectId), // (min-height, block, remote)
    PopBlock(u64, u64, ObjectId),   // (new-height, new-round, blockid)
}

pub struct Synchronizer {
    tx_sync_message: Sender<SynchronizerMessage>,
    network_sender: crate::network::Sender,
    rpath: GroupRPath,
}

impl Synchronizer {
    pub fn spawn(
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        height: u64,
        round: u64,
        tx_block: Sender<(HotstuffMessage, ObjectId)>,
    ) -> Self {
        let (tx_sync_message, rx_sync_message) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let runner = SynchronizerRunner::new(
            network_sender.clone(),
            rpath.clone(),
            tx_block,
            rx_sync_message,
            height,
            round,
        );

        async_std::task::spawn(async move || runner.run().await);

        Self {
            tx_sync_message,
            network_sender,
            rpath,
        }
    }

    pub fn sync_with_height(&self, min_height: u64, max_height: u64, remote: ObjectId) {
        if min_height > max_height {
            return Ok(());
        }

        let tx_sync_message = self.tx_sync_message.clone();
        async_std::task::spawn(async move || {
            tx_sync_message
                .send(SynchronizerMessage::Sync(
                    min_height,
                    SyncBound::Height(max_height),
                    remote,
                ))
                .await
        });
    }

    pub fn sync_with_round(&self, min_height: u64, max_round: u64, remote: ObjectId) {
        if min_height > max_round {
            return Ok(());
        }

        let tx_sync_message = self.tx_sync_message.clone();
        async_std::task::spawn(async move || {
            tx_sync_message
                .send(SynchronizerMessage::Sync(
                    min_height,
                    SyncBound::Round(max_round),
                    remote,
                ))
                .await
        });
    }

    pub fn push_outorder_block(
        &self,
        block: GroupConsensusBlock,
        min_height: u64,
        remote: ObjectId,
    ) {
        let tx_sync_message = self.tx_sync_message.clone();
        async_std::task::spawn(async move || {
            tx_sync_message
                .send(SynchronizerMessage::PushBlock(min_height, block, remote))
                .await
        });
    }

    pub fn pop_link_from(&self, block: &GroupConsensusBlock) {
        let tx_sync_message = self.tx_sync_message.clone();
        let height = block.height();
        let round = block.round();
        let block_id = block.named_object().desc().object_id();
        async_std::task::spawn(async move || {
            tx_sync_message
                .send(SynchronizerMessage::PopBlock(height, round, block_id))
                .await
        });
    }

    pub async fn process_sync_request(
        &self,
        min_bound: SyncBound,
        max_bound: SyncBound,
        remote: ObjectId,
        store: &Storage,
    ) -> BuckyResult<()> {
        let header_block = store.header_block();
        if header_block.is_none() {
            return Ok(());
        }

        let mut blocks = vec![];

        // map SyncBound::Round(x) to height, and collect the blocks found
        let header_block = header_block.as_ref().unwrap();
        let min_height = match min_bound {
            SyncBound::Round(round) => {
                if round > header_block.round() {
                    return Ok(());
                }

                // find the height
                let mut block = header_block.clone();
                let mut min_height = 1;
                let mut min_round = 1;
                let mut max_height = block.height();
                let mut max_round = block.round();

                while min_height < max_height {
                    match block.round().cmp(&round) {
                        std::cmp::Ordering::Equal => {
                            let pos = blocks
                                .binary_search_by(|b| b.round().cmp(&block.round()))
                                .unwrap_err();
                            blocks.insert(pos, block);
                            break;
                        }
                        std::cmp::Ordering::Less => {
                            min_round = block.round() + 1;
                            min_height = block.height() + 1;
                        }
                        std::cmp::Ordering::Greater => {
                            max_round = block.round() - 1;
                            max_height = block.height() - 1;

                            let is_include = match max_bound {
                                SyncBound::Round(max_round) => block.round() <= max_round,
                                SyncBound::Height(max_height) => block.height() <= max_height,
                            };
                            if is_include {
                                let pos = blocks
                                    .binary_search_by(|b| b.round().cmp(&block.round()))
                                    .unwrap_err();
                                blocks.insert(pos, block);
                            }
                        }
                    }

                    let height = min_height
                        + (round - min_round) * (max_height - min_height) / (max_round - min_round);

                    block = match store.get_block_by_height(height).await {
                        Ok(block) => block,
                        Err(_) => break,
                    }
                }

                if block.round() == round {
                    Some(block.height())
                } else {
                    None
                }
            }
            SyncBound::Height(height) => {
                if height > header_block.height() {
                    return Ok(());
                }

                Some(height)
            }
        };

        // load all blocks in [min_height, max_bound]
        // TODO: limit count
        if let Some(min_height) = min_height {
            for height in min_height..(header_block.height() + 1) {
                if let Err(pos) = blocks.binary_search_by(|b| b.height().cmp(&height)) {
                    if let Ok(block) = store.get_block_by_height(height).await {
                        let is_include = match max_bound {
                            SyncBound::Height(height) => block.height() <= height,
                            SyncBound::Round(round) => block.round() <= round,
                        };
                        if !is_include {
                            break;
                        }
                        blocks.insert(pos, block);
                    }
                }
            }
        }

        let network_sender = self.network_sender.clone();
        let rpath = self.rpath.clone();
        async_std::task::spawn(async move || {
            futures::future::join_all(blocks.into_iter().map(|block| {
                network_sender.post_package(HotstuffMessage::Block(block), rpath.clone(), &remote)
            }))
            .await;
        });

        Ok(())
    }
}

#[derive(Clone)]
struct ResendInfo {
    last_send_time: Instant,
    send_times: usize,
    cmd: Arc<(u64, SyncBound, ObjectId)>,
}

#[derive(Clone)]
struct RequestSendInfo {
    min_bound: SyncBound,
    max_bound: SyncBound,

    resends: Vec<ResendInfo>,
}

impl RequestSendInfo {
    fn new(
        min_bound: SyncBound,
        max_bound: SyncBound,
        req: Arc<(u64, SyncBound, ObjectId)>,
    ) -> Self {
        RequestSendInfo {
            min_bound,
            max_bound: max_bound,
            resends: vec![ResendInfo {
                last_send_time: Instant::now(),
                send_times: 0,
                cmd: req,
            }],
        }
    }

    fn splite(&mut self, bound: SyncBound) -> Option<Self> {
        match bound.cmp(&self.max_bound) {
            std::cmp::Ordering::Greater => None,
            _ => match bound.cmp(&self.min_bound) {
                std::cmp::Ordering::Greater => {
                    self.max_bound = bound.sub(1);
                    Some(Self {
                        min_bound: bound,
                        max_bound: self.max_bound,
                        resends: self.resends.clone(),
                    })
                }
                _ => None,
            },
        }
    }

    fn try_send(&mut self, rpath: GroupRPath, sender: &crate::network::Sender) {
        // 选send次数最少，间隔最长的发送一次
        if let SyncBound::Round(_) = self.min_bound {
            return;
        }

        let now = Instant::now();
        let max_send_info_pos = 0;
        for i in 1..self.resends.len() {
            let resend_info = self.resends.get(i).unwrap();
            let max_send_info = self.resends.get(max_send_info_pos).unwrap();

            if now.duration_since(resend_info.last_send_time)
                <= Duration::from_millis(SYNCHRONIZER_TIMEOUT * (1 << resend_info.send_times))
            {
                return;
            }
            match resend_info.send_times.cmp(&max_send_info.send_times) {
                std::cmp::Ordering::Less => {
                    max_send_info_pos = i;
                }
                std::cmp::Ordering::Greater => {}
                std::cmp::Ordering::Equal => {
                    if let std::cmp::Ordering::Greater = now
                        .duration_since(resend_info.last_send_time)
                        .cmp(&now.duration_since(max_send_info.last_send_time))
                    {
                        max_send_info_pos = i;
                    }
                }
            }
        }

        if let Some(resend_info) = self.resends.get_mut(max_send_info_pos) {
            resend_info.last_send_time = now;
            resend_info.send_times += 1;

            let msg = HotstuffMessage::SyncRequest(self.min_bound, self.max_bound);
            let remote = resend_info.cmd.2;
            async_std::task::spawn(async move || sender.post_package(msg, rpath, &remote).await);

            if resend_info.send_times >= SYNCHRONIZER_TRY_TIMES {
                self.resends.remove(max_send_info_pos);
            }
        }
    }

    fn is_valid(&self) -> bool {
        if let std::cmp::Ordering::Greater = self.min_bound.cmp(&self.max_bound) {
            false
        } else {
            true
        }
    }
}

struct SynchronizerRunner {
    network_sender: crate::network::Sender,
    rpath: GroupRPath,
    tx_block: Sender<(HotstuffMessage, ObjectId)>,
    rx_message: Receiver<(HotstuffMessage, ObjectId)>,
    timer: Timer,
    height: u64,
    round: u64,

    sync_requests: Vec<RequestSendInfo>, // order by min_bound
    out_order_blocks: Vec<(GroupConsensusBlock, ObjectId)>, // Vec<(block, remote)>
}

impl SynchronizerRunner {
    fn new(
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        tx_block: Sender<(HotstuffMessage, ObjectId)>,
        rx_message: Receiver<SynchronizerMessage>,
        height: u64,
        round: u64,
    ) -> Self {
        Self {
            network_sender,
            rpath,
            rx_message,
            timer: Timer::new(SYNCHRONIZER_TIMEOUT),
            height,
            round,
            sync_requests: vec![],
            out_order_blocks: vec![],
            tx_block,
        }
    }

    async fn handle_sync(&mut self, min_height: u64, max_bound: SyncBound, remote: ObjectId) {
        let min_height = min_height.max(self.height + 1);
        let max_bound = match max_bound {
            SyncBound::Height(height) => SyncBound::Height(height.max(self.height + 1)),
            SyncBound::Round(round) => SyncBound::Round(round.max(self.round + 1)),
        };

        let requests: Vec<Arc<(u64, SyncBound, ObjectId)>> = self
            .filter_outorder_blocks(min_height, max_bound)
            .into_iter()
            .map(|req| Arc::new((req.0, req.1, remote)))
            .collect();

        // combine requests
        let mut pos = 0;
        for req in requests {
            let mut range = (SyncBound::Height(req.0), req.1);
            while range.0 <= range.1 {
                while pos < self.sync_requests.len() {
                    let req1 = self.sync_requests.get_mut(pos).unwrap();
                    match range.0.cmp(&req1.min_bound) {
                        std::cmp::Ordering::Less => {
                            let max_bound = match range.1.cmp(&req1.min_bound) {
                                std::cmp::Ordering::Less => range.1,
                                _ => req1.min_bound.sub(1),
                            };

                            let new_req = RequestSendInfo::new(range.0, max_bound, req.clone());
                            new_req.try_send(self.rpath.clone(), &self.network_sender);
                            self.sync_requests.insert(i, new_req);
                            range.0 = max_bound.value() + 1;
                        }
                        std::cmp::Ordering::Equal => {
                            match range.1.cmp(&req1.max_bound) {
                                std::cmp::Ordering::Greater => {
                                    range.0 = req1.max_bound.add(1);
                                }
                                _ => {
                                    range.0 = range.1.add(1);
                                    let cut = req1.splite(range.0);
                                    assert!(req1.is_valid());
                                    if let Some(cut) = cut {
                                        self.sync_requests.insert(i + 1, cut);
                                    }
                                }
                            };
                            req1.resends.push(ResendInfo {
                                last_send_time: 0,
                                send_times: 0,
                                cmd: req.clone(),
                            });
                        }
                        std::cmp::Ordering::Greater => match range.0.cmp(&req1.max_bound) {
                            std::cmp::Ordering::Greater => {}
                            _ => {
                                let cut = req1.splite(range.0);
                                assert!(req1.is_valid());
                                if let Some(cut) = cut {
                                    self.sync_requests.insert(i + 1, cut);
                                }
                            }
                        },
                    }
                    pos += 1;

                    if range.0 > range.1 {
                        break;
                    }
                }

                if pos == self.sync_requests.len() {
                    if range.0 <= range.1 {
                        let new_req = RequestSendInfo::new(range.0, max_bound, req.clone());
                        new_req.try_send(self.rpath.clone(), &self.network_sender);
                        self.sync_requests.push(new_req);
                        pos += 1;
                    }
                    break;
                }
            }
        }
    }

    fn filter_outorder_blocks(
        &self,
        mut min_height: u64,
        mut max_bound: SyncBound,
    ) -> Vec<(u64, SyncBound)> {
        // TODO: limit the lenght of per range
        let mut last_range = Some((SyncBound::Height(min_height), max_bound));
        let mut requests = vec![];
        for (block, _) in self.out_order_blocks.as_slice() {
            match last_range {
                Some(range) => {
                    let (range1, range2) =
                        Self::splite_range_with_block(range, block.height(), block.round());
                    if let Some(range1) = range1 {
                        requests.push(range1);
                    }
                    last_range = range2;
                }
                None => break,
            }
        }

        if let Some(last_range) = last_range {
            requests.push(last_range);
        }

        requests
    }

    fn splite_range_with_block(
        mut range: (SyncBound, SyncBound),
        height: u64,
        round: u64,
    ) -> (
        Option<(SyncBound, SyncBound)>,
        Option<(SyncBound, SyncBound)>,
    ) {
        let min_ord = match range.0 {
            SyncBound::Height(height) => height.cmp(&height),
            SyncBound::Round(round) => round.cmp(&round),
        };

        match min_ord {
            std::cmp::Ordering::Less => (None, Some((range.0, range.1))),
            std::cmp::Ordering::Equal => {
                range.0 = range.0.add(1);
                match range.0.cmp(&range.1) {
                    std::cmp::Ordering::Greater => (None, None),
                    _ => (None, Some((range.0, range.1))),
                }
            }
            std::cmp::Ordering::Greater => {
                let ord = match range.1 {
                    SyncBound::Height(height) => height.cmp(&height),
                    SyncBound::Round(round) => round.cmp(&round),
                };

                match ord {
                    std::cmp::Ordering::Less => (
                        Some((range.0, SyncBound::Height(height - 1))),
                        Some((SyncBound::Height(height + 1), range.1)),
                    ),
                    std::cmp::Ordering::Equal => {
                        (Some((range.0, SyncBound::Height(height - 1))), None)
                    }
                    std::cmp::Ordering::Greater => (Some((range.0, range.1)), None),
                }
            }
        }
    }

    async fn handle_push_block(
        &mut self,
        min_height: u64,
        block: GroupConsensusBlock,
        remote: ObjectId,
    ) {
        if block.round() <= self.round {
            return;
        }

        if min_height >= block.height() {
            return;
        }

        let pos = self.out_order_blocks.binary_search_by(|(block0, _)| {
            let ord = block0.height().cmp(&block.height());
            if let std::cmp::Ordering::Equal = ord {
                block0.round().cmp(&block.round())
            } else {
                ord
            }
        });

        match pos {
            Ok(_) => return,
            Err(pos) => self.out_order_blocks.insert(pos, (block, remote)),
        };

        self.timer.reset(SYNCHRONIZER_TIMEOUT);

        for i in 0..self.sync_requests.len() {
            let req = self.sync_requests.get_mut(i).unwrap();
            let (range1, range2) = Self::splite_range_with_block(
                (req.min_bound, req.max_bound),
                block.height(),
                block.round(),
            );
            match range1 {
                Some(range1) => {
                    req.max_bound = range1.1;
                    if let Some(range2) = range2 {
                        let mut new_req = req.clone();
                        new_req.min_bound = range2.0;
                        new_req.max_bound = range2.1;
                        self.sync_requests.insert(i + 1, new_req);
                        break;
                    }
                }
                None => {
                    match range2 {
                        Some(range2) => req.min_bound = range2.0,
                        None => self.sync_requests.remove(i),
                    }
                    break;
                }
            }
        }

        self.handle_sync(min_height, SyncBound::Height(block.height()), remote)
            .await;
    }

    async fn handle_pop_block(&mut self, new_height: u64, new_round: u64, block_id: ObjectId) {
        if new_round <= self.round {
            return;
        }

        self.timer.reset(SYNCHRONIZER_TIMEOUT);

        let mut max_height = self.height.max(&new_height);
        let mut max_round = new_round;

        let mut remove_block_ids = HashSet::from(&[block_id]);

        let mut remove_pos = None;

        for pos in 0..self.out_order_blocks.len() {
            let (block, remote) = self.out_order_blocks.get(pos).unwrap();

            let block_id_out = block.named_object().desc().object_id();
            if remove_block_ids.contains(&block.prev_block_id()) || block_id_out == block_id {
                remove_block_ids.insert(block_id_out);
                remove_pos = Some(pos);
                max_height = max_height.max(block.height());
                max_round = max_round.max(block.round());
            } else if block.height() > max_height && block.round() > max_round {
                break;
            }
        }

        let order_blocks = match remove_pos {
            Some(remove_pos) => self
                .out_order_blocks
                .splice(0..(remove_pos + 1), [])
                .collect(),
            None => vec![],
        };

        self.height = max_height;
        self.round = max_round;

        let mut remove_request_pos = None;
        for pos in 0..self.sync_requests.len() {
            let req = self.sync_requests.get_mut(pos).unwrap();
            let (first, second) = Self::splite_range_with_block(
                (req.min_bound, req.max_bound),
                self.height,
                self.round,
            );
            match first {
                Some(first) => {
                    remove_request_pos = Some(pos);
                    req.max_bound = first.1;
                    if let Some(second) = second {
                        let mut new_req = req.clone();
                        new_req.min_bound = second.0;
                        new_req.max_bound = second.1;
                        self.sync_requests.insert(pos + 1, new_req);
                        break;
                    }
                }
                None => {
                    if let Some(second) = second {
                        req.min_bound = second.0;
                    }
                    break;
                }
            };
        }

        if let Some(remove_request_pos) = remove_request_pos {
            self.sync_requests.splice(0..(pos + 1), []);
        }

        futures::future::join_all(order_blocks.into_iter().map(|(order_block, remote)| {
            self.tx_block
                .send((HotstuffMessage::Block(order_block), remote))
        }))
        .await;
    }

    async fn handle_timeout(&mut self) {
        self.sync_requests.retain_mut(|req| {
            req.try_send(self.rpath.clone(), &self.network_sender);
            req.resends.len() > 0
        });
    }

    async fn run(&mut self) {
        loop {
            futures::select! {
                message = self.rx_message.recv().fuse() => match message {
                    Ok(SynchronizerMessage::Sync(min_height, max_bound, remote)) => self.handle_sync(min_height, max_bound, remote).await,
                    Ok(SynchronizerMessage::PushBlock(min_height, block, remote)) => self.handle_push_block(min_height, block, remote).await,
                    Ok(SynchronizerMessage::PopBlock(new_height, new_round, block_id)) => self.handle_pop_block(new_height, new_round, block_id).await,
                    Err(e) => {
                        log::warn!("[synchronizer] rx_message closed.");
                        Ok(())
                    },
                },
                () = self.timer.wait_next().fuse() => self.handle_timeout().await,
            };
        }
    }
}
