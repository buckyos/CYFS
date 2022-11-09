
use std::{sync::{Arc, RwLock}, collections::{BTreeMap, }};

use cyfs_base::{DeviceId, BuckyErrorCode};
use cyfs_util::{SqliteStorage, AsyncStorage};

use crate::{Timestamp, TempSeq};

mod manager;
mod storage;

pub use manager::StatisticManager;

#[derive(Clone, Debug)]
pub enum PeerStatusKind {
    Connecting(Timestamp /* start timestamp */),
    Online(Timestamp /* start timestamp */, Timestamp /* online timestamp */),
}

impl std::fmt::Display for PeerStatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            PeerStatusKind::Connecting(start_stamp) => {
                write!(f, "({} connecting)", start_stamp)
            }
            PeerStatusKind::Online(start_stamp, online_stamp) => {
                write!(f, "({} connecting {} online)", start_stamp, online_stamp)
            }
        }
    }
}

struct CallException {
    peer_id: DeviceId, 
    seq: TempSeq,
}

struct CallFailure {
    peer_id: DeviceId, 
    seq: TempSeq,
    errno: BuckyErrorCode,
}

struct PeerStatusImpl {
    id: DeviceId,
    status: PeerStatusKind,

    records: BTreeMap<(DeviceId, TempSeq), StatusKind>,

    will_cache_record: Vec<(Option<DeviceId>, Option<TempSeq>, StatusKind)>,
}

#[derive(Debug)]
enum StatusKind {
    OnlineResult(PeerStatusKind),
    CallResult(BuckyErrorCode),
}

impl std::fmt::Display for StatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::OnlineResult(k) => {
                k.fmt(f)
            }
            Self::CallResult(r) => {
                write!(f, "(call result {})", r)
            }
        }
    }
}

// struct CallResult {
//     errno: BuckyErrorCode,
// }

// impl std::default::Default for CallResult {
//     fn default() -> Self {
//         Self { errno: BuckyErrorCode::Ok }
//     }
// }

#[derive(Clone)]
pub struct PeerStatus(Arc<RwLock<PeerStatusImpl>>);

impl PeerStatus {
    pub fn new(id: DeviceId, now: Timestamp) -> Self {
        Self(Arc::new(RwLock::new(PeerStatusImpl {
            id,
            status: PeerStatusKind::Connecting(now),
            records: BTreeMap::new(),
            will_cache_record: Vec::new(),
        })))
    }

    pub fn wait_online(&self, now: Timestamp, recod: bool) {
        let status = &mut *self.0.write().unwrap();

        match status.status {
            PeerStatusKind::Online(_, _) => {
                status.status = PeerStatusKind::Connecting(now);
            }
            PeerStatusKind::Connecting(start_stamp) => {
                if recod {
                    status.will_cache_record.push((None, None, StatusKind::OnlineResult(status.status.clone())));
                    status.status = PeerStatusKind::Connecting(now);
                } else {
                    // Ignore
                }
            }
        }
    }

    // pub fn status(&self) -> PeerStatusKind {
    //     self.0.read().unwrap().status.clone()
    // }

    pub fn online(&self, seq: TempSeq, now: Timestamp) {
        let status = &mut *self.0.write().unwrap();

        match status.status {
            PeerStatusKind::Online(start_stamp, online_stamp) => {
                // Ignore, continue
            }
            PeerStatusKind::Connecting(start_stamp) => {

                status.status = PeerStatusKind::Online(start_stamp, now);
                status.will_cache_record.push((None, Some(seq), StatusKind::OnlineResult(status.status.clone())));
            }
        }
        // match status.status {
        //     PeerStatusKind::Connecting(start_timestamp) => {
        //         status.status = PeerStatusKind::Online(start_timestamp, now);
        //     }
        //     _ => {}
        // }
    }

    pub fn add_record(&self, peer_id: DeviceId, seq: TempSeq) {
        self.0.write().unwrap()
            .records
            .entry((peer_id, seq))
            .or_insert(StatusKind::CallResult(BuckyErrorCode::Ok));
    }

    pub fn record(&self, peer_id: DeviceId, seq: TempSeq, errno: BuckyErrorCode) {

        let w = &mut *self.0.write().unwrap();
        match w.records
               .remove_entry(&(peer_id.clone(), seq)) {
            Some(((peer_id, seq), _)) => {
                w.will_cache_record.push((Some(peer_id), Some(seq), StatusKind::CallResult(errno)))
            }
            None => {
                warn!("not found {}-{} status", peer_id, seq.value());
            }
        }
        // match self.0.write().unwrap()
        //           .call_record
        //           .entry((peer_id, seq)) {
        //     Entry::Occupied(mut exist) => {
        //         exist.get_mut().errno = errno;
        //     }
        //     Entry::Vacant(not_found) => {
        //         not_found.insert(CallResult{
        //             errno
        //         });
        //     }
        // }
    }
    // pub fn call_exception(&mut self, peer_id: DeviceId, seq: TempSeq) {
    //     self.0.write().unwrap()
    //         .call_exception
    //         .push(CallException{
    //             peer_id, seq,
    //         });
    // }

    // pub fn call_failure(&mut self, peer_id: DeviceId, seq: TempSeq, errno: BuckyErrorCode) {
    //     self.0.write().unwrap()
    //         .call_failure
    //         .push(CallFailure{
    //             peer_id, 
    //             seq,
    //             errno,
    //         });
    // }
}

impl PeerStatus {
    pub async fn storage(&self, storage: &mut SqliteStorage) {
        let mut remove_recode = vec![];

        let self_id = {
            let w = &mut self.0.write().unwrap();
            let will_cache_record = &mut w.will_cache_record;
            println!("==============={:#?}", will_cache_record);

            if will_cache_record.len() > 0 {
                std::mem::swap(&mut remove_recode, will_cache_record);
                // unsafe { std::ptr::swap(remove_recode.as_mut_ptr(), will_cache_record.as_mut_ptr()) };
            }

            w.id.clone()
        };

        for (to_peer, seq, status) in remove_recode.iter() {
            let to_result = status.to_string();

            let key_id = if let Some(to_peer) = to_peer {
                to_peer
            } else {
                &self_id
            };

            let key = if let Some(seq) = seq {
                format!("{}-{}", key_id, seq.value())
            } else {
                format!("{}", key_id)
            };

            let _ = storage.set_item(key.as_str(), to_result).await;
        }
    }
}
