use std::{
    time::Duration, 
    collections::BTreeMap, 
};
use cyfs_base::*;
use crate::{
    types::*
};
use cyfs_debug::Mutex;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RemoteSeq {
    remote: DeviceId, 
    seq: TempSeq, 
}

struct StubImpl {
    last_recycle: Timestamp, 
    stubs: BTreeMap<RemoteSeq, Timestamp>
}
pub struct CallStub(Mutex<StubImpl>);

impl CallStub {
    pub fn new() -> Self {
        Self(Mutex::new(StubImpl {
            last_recycle: bucky_time_now(), 
            stubs: Default::default()
        }))
    }

    pub fn insert(&self, remote: &DeviceId, seq: &TempSeq) -> bool {
        let remote_seq = RemoteSeq {
            remote: remote.clone(),
            seq: *seq
        };
        let stubs = &mut self.0.lock().unwrap().stubs;
        stubs.insert(remote_seq, bucky_time_now()).is_none()
    }

    pub fn recycle(&self, now: Timestamp) {
        let mut stub = self.0.lock().unwrap();
        if now > stub.last_recycle && Duration::from_micros(now - stub.last_recycle) > Duration::from_secs(60) {
            let mut to_remove = vec![];
            for (key, when) in stub.stubs.iter() {
                if now > *when && Duration::from_micros(now - *when) > Duration::from_secs(60) {
                    to_remove.push(key.clone());
                }
            }
            for key in to_remove {
                stub.stubs.remove(&key);
            }
            stub.last_recycle = bucky_time_now();
        }
    }
}


