use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
    vec,
};

use async_std::channel::{Receiver, Sender};
use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, ObjectId};
use cyfs_core::{GroupConsensusBlock, GroupConsensusBlockObject};

use crate::{consensus::timer::Timer, HotstuffMessage, SYNCHRONIZER_TIMEOUT};

enum SyncMaxBound {
    Height(u64),
    Round(u64),
}

impl SyncMaxBound {
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

impl Ord for SyncMaxBound {
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
    Sync(u64, SyncMaxBound, ObjectId), // ([min-height, max-bound], remote)
    PushBlock(u64, GroupConsensusBlock, ObjectId), // (min-height, block, remote)
    PopBlock(u64, u64, ObjectId),      // (new-height, new-round, blockid)
}

pub struct Synchronizer {}

impl Synchronizer {
    pub fn spawn(
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        height: u64,
        round: u64,
        tx_block: Sender<(HotstuffMessage, ObjectId)>,
    ) -> Self {
        Self {}
    }

    pub fn sync_with_height(
        &self,
        min_height: u64,
        max_height: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        if min_height > max_height {
            return Ok(());
        }
        unimplemented!()
    }

    pub fn sync_with_round(
        &self,
        min_height: u64,
        max_round: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn push_outorder_block(
        &self,
        block: GroupConsensusBlock,
        min_height: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn pop_link_from(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
    }
}

#[derive(Clone)]
struct ResendInfo {
    last_send_time: Instant,
    send_times: usize,
    cmd: Arc<(u64, SyncMaxBound, ObjectId)>,
}

#[derive(Clone)]
struct RequestSendInfo {
    min_bound: SyncMaxBound,
    max_bound: SyncMaxBound,

    resends: Vec<ResendInfo>,
}

impl RequestSendInfo {
    fn new(
        min_bound: SyncMaxBound,
        max_bound: SyncMaxBound,
        req: Arc<(u64, SyncMaxBound, ObjectId)>,
    ) -> Self {
        RequestSendInfo {
            min_bound,
            max_bound: max_bound,
            resends: vec![ResendInfo {
                last_send_time: Instant::now(),
                send_times: 1,
                cmd: req,
            }],
        }
    }

    fn splite(&mut self, bound: SyncMaxBound) -> Option<Self> {
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

    fn try_send(&mut self) {
        // todo 选send次数最少，间隔最长的发送一次
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

    async fn handle_sync(&mut self, min_height: u64, max_bound: SyncMaxBound, remote: ObjectId) {
        let min_height = min_height.max(self.height + 1);
        let max_bound = match max_bound {
            SyncMaxBound::Height(height) => SyncMaxBound::Height(height.max(self.height + 1)),
            SyncMaxBound::Round(round) => SyncMaxBound::Round(round.max(self.round + 1)),
        };

        let requests: Vec<Arc<(u64, SyncMaxBound, ObjectId)>> = self
            .filter_outorder_blocks(min_height, max_bound)
            .into_iter()
            .map(|req| Arc::new((req.0, req.1, remote)))
            .collect();

        // combine requests
        let mut pos = 0;
        for req in requests {
            let mut range = (SyncMaxBound::Height(req.0), req.1);
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
                            new_req.try_send();
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
                        new_req.try_send();
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
        mut max_bound: SyncMaxBound,
    ) -> Vec<(u64, SyncMaxBound)> {
        let mut last_range = Some((SyncMaxBound::Height(min_height), max_bound));
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
        mut range: (SyncMaxBound, SyncMaxBound),
        height: u64,
        round: u64,
    ) -> (
        Option<(SyncMaxBound, SyncMaxBound)>,
        Option<(SyncMaxBound, SyncMaxBound)>,
    ) {
        let min_ord = match range.0 {
            SyncMaxBound::Height(height) => height.cmp(&height),
            SyncMaxBound::Round(round) => round.cmp(&round),
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
                    SyncMaxBound::Height(height) => height.cmp(&height),
                    SyncMaxBound::Round(round) => round.cmp(&round),
                };

                match ord {
                    std::cmp::Ordering::Less => (
                        Some((range.0, SyncMaxBound::Height(height - 1))),
                        Some((SyncMaxBound::Height(height + 1), range.1)),
                    ),
                    std::cmp::Ordering::Equal => {
                        (Some((range.0, SyncMaxBound::Height(height - 1))), None)
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

        self.handle_sync(min_height, SyncMaxBound::Height(block.height()), remote)
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
        for req in self.sync_requests.iter() {
            req.try_send();
        }
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
