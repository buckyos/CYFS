
use std::{sync::{Arc, RwLock}, collections::{BTreeMap, }};

use cyfs_base::{DeviceId, BuckyErrorCode, SocketAddr};
use cyfs_util::{SqliteStorage, AsyncStorage};

use crate::{Timestamp, TempSeq};

mod manager;

pub use manager::StatisticManager;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum StatisticKey {
    RemoteId(DeviceId),
    RemoteEndpoint(SocketAddr),
}

impl std::fmt::Display for StatisticKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::RemoteId(id) => {
                write!(f, "{}", id)
            }
            Self::RemoteEndpoint(addr) => {
                write!(f, "{}", addr.to_string())
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum PeerStatusKind {
    NoRecod,
    Connecting(Timestamp /* start timestamp */),
    Online(Timestamp /* start timestamp */, Timestamp /* online timestamp */),
}

impl std::fmt::Display for PeerStatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            PeerStatusKind::NoRecod => {
                Ok(())
            },
            PeerStatusKind::Connecting(start_stamp) => {
                write!(f, "({} connecting)", start_stamp)
            }
            PeerStatusKind::Online(start_stamp, online_stamp) => {
                write!(f, "({} connecting {} online)", start_stamp, online_stamp)
            }
        }
    }
}

struct PeerStatusImpl {
    id: StatisticKey,
    status: PeerStatusKind,

    records: BTreeMap<(StatisticKey, TempSeq), StatusKind>,

    will_cache_record: Vec<(StatisticKey, Option<TempSeq>, StatusKind)>,
}

#[derive(Debug)]
enum StatusKind {
    OnlineResult(PeerStatusKind),
    CallResult(BuckyErrorCode),
    ErrorResult(String),
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
            Self::ErrorResult(r) => {
                write!(f, "(error result {})", r)
            }
        }
    }
}

#[derive(Clone)]
pub struct PeerStatus(Arc<RwLock<PeerStatusImpl>>);

impl PeerStatus {
    pub fn with_peer(id: DeviceId, now: Timestamp) -> Self {
        Self(Arc::new(RwLock::new(PeerStatusImpl {
            id: StatisticKey::RemoteId(id),
            status: PeerStatusKind::Connecting(now),
            records: BTreeMap::new(),
            will_cache_record: Vec::new(),
        })))
    }

    pub fn with_endpoint(ep: SocketAddr) -> Self {
        Self(Arc::new(RwLock::new(PeerStatusImpl {
            id: StatisticKey::RemoteEndpoint(ep),
            status: PeerStatusKind::NoRecod,
            records: BTreeMap::new(),
            will_cache_record: Vec::new(),
        })))
    }

    pub fn wait_online(&self, now: Timestamp, recod: bool) {
        let status = &mut *self.0.write().unwrap();

        match status.status {
            PeerStatusKind::NoRecod => {},
            PeerStatusKind::Online(_, _) => {
                status.status = PeerStatusKind::Connecting(now);
            }
            PeerStatusKind::Connecting(_start_stamp) => {
                if recod {
                    status.will_cache_record.push((status.id.clone(), None, StatusKind::OnlineResult(status.status.clone())));
                    status.status = PeerStatusKind::Connecting(now);
                } else {
                    // Ignore
                }
            }
        }
    }

    pub fn online(&self, seq: TempSeq, now: Timestamp) {
        let status = &mut *self.0.write().unwrap();

        match status.status {
            PeerStatusKind::NoRecod => {},
            PeerStatusKind::Online(_start_stamp, _online_stamp) => {
                // Ignore, continue
            }
            PeerStatusKind::Connecting(start_stamp) => {

                status.status = PeerStatusKind::Online(start_stamp, now);
                status.will_cache_record.push((status.id.clone(), Some(seq), StatusKind::OnlineResult(status.status.clone())));
            }
        }
    }

    pub fn add_record(&self, peer_id: DeviceId, seq: TempSeq) {
        self.0.write().unwrap()
            .records
            .entry((StatisticKey::RemoteId(peer_id), seq))
            .or_insert(StatusKind::CallResult(BuckyErrorCode::Ok));
    }

    pub fn record(&self, peer_id: DeviceId, seq: TempSeq, errno: BuckyErrorCode) {

        let w = &mut *self.0.write().unwrap();
        match w.records
               .remove_entry(&(StatisticKey::RemoteId(peer_id), seq)) {
            Some(((peer_id, seq), _)) => {
                w.will_cache_record.push((peer_id, Some(seq), StatusKind::CallResult(errno)))
            }
            None => {}
        }

    }

    pub fn recod_error(&self, error_message: String) {
        let w = &mut *self.0.write().unwrap();
        w.will_cache_record.push((w.id.clone(), None, StatusKind::ErrorResult(error_message)));
    }
}

impl PeerStatus {
    pub async fn storage(&self, storage: &mut SqliteStorage) {
        let mut remove_recode = vec![];

        {
            let w = &mut self.0.write().unwrap();
            let will_cache_record = &mut w.will_cache_record;

            if will_cache_record.len() > 0 {
                std::mem::swap(&mut remove_recode, will_cache_record);
            }
        }

        for (to_peer, seq, status) in remove_recode.iter() {
            let to_result = status.to_string();

            let key_id = to_peer;

            let key = if let Some(seq) = seq {
                format!("{}-{}", key_id, seq.value())
            } else {
                format!("{}", key_id)
            };

            let _ = storage.set_item(key.as_str(), to_result).await;
        }
    }
}
