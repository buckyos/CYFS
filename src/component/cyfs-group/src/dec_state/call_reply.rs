use std::{collections::HashMap, sync::Arc};

use async_std::sync::{Mutex, RwLock};

use crate::CHANNEL_CAPACITY;

type CallSeq = u64;

pub struct CallReplyWaiter<T> {
    rx: async_std::channel::Receiver<T>,
    seq: CallSeq,
    container: Arc<Mutex<HashMap<CallSeq, async_std::channel::Sender<T>>>>,
}

impl<T> CallReplyWaiter<T> {
    pub fn wait(&self) -> async_std::channel::Recv<'_, T> {
        self.rx.recv()
    }
}

impl<T> Drop for CallReplyWaiter<T> {
    fn drop(&mut self) {
        let container = self.container.clone();
        async_std::task::block_on(async move {
            let mut container = container.lock().await;
            container.remove(&self.seq);
        });
    }
}

struct CallReplyNotifierRaw<K: std::hash::Hash + std::cmp::Eq, T> {
    next_seq: CallSeq,
    senders: Arc<Mutex<HashMap<CallSeq, async_std::channel::Sender<T>>>>,
    call_keys: HashMap<K, Vec<CallSeq>>,
}

#[derive(Clone)]
pub struct CallReplyNotifier<K: std::hash::Hash + std::cmp::Eq, T>(
    Arc<RwLock<CallReplyNotifierRaw<K, T>>>,
);

impl<K: std::hash::Hash + std::cmp::Eq + std::fmt::Debug, T: Clone> CallReplyNotifier<K, T> {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(CallReplyNotifierRaw {
            next_seq: 1,
            senders: Arc::new(Mutex::new(HashMap::new())),
            call_keys: HashMap::new(),
        })))
    }

    pub async fn prepare(&self, k: K) -> CallReplyWaiter<T> {
        let (tx, rx) = async_std::channel::bounded(CHANNEL_CAPACITY);
        let mut mgr = self.0.write().await;
        let seq = mgr.next_seq;
        mgr.next_seq += 1;
        mgr.senders.lock().await.insert(seq, tx);
        mgr.call_keys.entry(k).or_insert_with(Vec::new).push(seq);

        CallReplyWaiter {
            rx,
            seq,
            container: mgr.senders.clone(),
        }
    }

    pub async fn reply(&self, key: &K, value: T) {
        let (abort_calls, senders) = {
            let mgr = self.0.read().await;
            let senders = mgr.senders.lock().await;
            match mgr.call_keys.get(key) {
                Some(call_seqs) => {
                    let mut valid_senders = vec![];
                    let mut abort_calls = vec![];
                    for call_seq in call_seqs {
                        match senders.get(call_seq) {
                            Some(sender) => valid_senders.push(sender.clone()),
                            None => abort_calls.push(*call_seq),
                        }
                    }

                    (abort_calls, valid_senders)
                }
                None => return,
            }
        };

        if abort_calls.len() > 0 {
            let mut mgr = self.0.write().await;
            if let Some(call_seqs) = mgr.call_keys.get_mut(key) {
                call_seqs.retain(|seq| !abort_calls.contains(seq));
                if call_seqs.len() == 0 {
                    mgr.call_keys.remove(key);
                }
            }
        }

        for sender in senders {
            if let Err(err) = sender.send(value.clone()).await {
                log::warn!("reply to caller failed, key: {:?},  err: {:?}.", key, err);
            }
        }
    }
}
