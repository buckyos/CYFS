use crate::state::*;
use cyfs_base::*;

use std::ops::Range;
use std::sync::{Arc, Mutex};

pub struct PoWStateManagerInner {
    active_threads: Vec<u32>,
    state: PoWState,
}

impl PoWStateManagerInner {
    pub fn new(object_id: ObjectId, difficulty: u8, id_range: Range<u32>) -> Self {
        assert!(!id_range.is_empty());
        Self {
            active_threads: vec![],
            state: PoWState::new(object_id, difficulty, id_range),
        }
    }

    pub async fn load_or_new(
        object_id: ObjectId,
        difficulty: u8,
        id_range: Range<u32>,
        storage: PoWStateStorageRef,
    ) -> BuckyResult<Self> {
        let data = PoWData {
            object_id,
            difficulty,
            nonce: None,
        };

        let ret = match storage.load(&data).await? {
            Some(state) => Self {
                active_threads: vec![],
                state,
            },
            None => Self::new(object_id, difficulty, id_range),
        };

        Ok(ret)
    }

    fn update_thread_state(&mut self, state: &PoWThreadState) {
        if let Some(thread) = self
            .state
            .threads
            .iter_mut()
            .find(|item| item.id == state.id)
        {
            // Got result!
            if state.data.nonce.is_some() {
                info!("pow got result! data={:?}, thread={}", state.data, state.id);

                self.state.data = state.data.clone();
            }

            info!("pow thread stated updated: id={}, {:?} -> {:?}", state.id, thread.range, state.range);
            *thread = state.to_owned();
        } else {
            error!("sync pow thread state but not found! state={:?}", state);
        }
    }

    fn sync_thread(&mut self, state: &PoWThreadState, status: PowThreadStatus) -> bool {
        match status {
            PowThreadStatus::Sync => {
                self.update_thread_state(state);
                self.state.data.nonce.is_none()
            }
            PowThreadStatus::Finished => {
                info!("pow thread complete, state={:?}", state);
                self.update_thread_state(state);

                if !self.state.finished.insert(state.id) {
                    error!("pow thread finished but already exists! id={}", state.id);
                }

                if let Some(index) = self.active_threads.iter().find(|item| **item == state.id) {
                    self.active_threads.remove(*index as usize);
                } else {
                    error!(
                        "pow thread finished state but not found in active threads! state={:?}",
                        state
                    );
                }

                self.state.data.nonce.is_none()
            }
        }
    }

    fn select_thread(&mut self) -> Option<PoWThreadState> {
        if let Some(thread) = self
            .state
            .threads
            .iter()
            .find(|item| !self.is_active(item.id))
        {
            assert!(!self.is_finished(thread.id));

            self.active_threads.push(thread.id);
            return Some(thread.to_owned());
        }

        match self.state.id_range.clone().find(|id| !self.is_exists(*id) && !self.is_finished(*id)) {
            Some(id) => {
                let thread = PoWThreadState::new(self.state.data.clone(), id);
                self.state.threads.push(thread.clone());
                self.active_threads.push(id);
                Some(thread)
            }
            None => None,
        }
    }

    fn is_active(&self, id: u32) -> bool {
        self.active_threads
            .iter()
            .find(|item| **item == id)
            .is_some()
    }

    fn is_finished(&self, id: u32) -> bool {
        self.state.finished.contains(&id)
    }

    fn is_exists(&self, id: u32) -> bool {
        self.state
            .threads
            .iter()
            .find(|item| item.id == id)
            .is_some()
    }
}

#[derive(Clone)]
pub struct PoWStateManager(Arc<Mutex<PoWStateManagerInner>>);

impl PoWStateManager {
    pub fn new(object_id: ObjectId, difficulty: u8, id_range: Range<u32>) -> Self {
        Self(Arc::new(Mutex::new(PoWStateManagerInner::new(
            object_id, difficulty, id_range,
        ))))
    }

    pub async fn load_or_new(
        object_id: ObjectId,
        difficulty: u8,
        id_range: Range<u32>,
        storage: PoWStateStorageRef,
    ) -> BuckyResult<Self> {
        Ok(Self(Arc::new(Mutex::new(
            PoWStateManagerInner::load_or_new(object_id, difficulty, id_range, storage).await?,
        ))))
    }
}

#[async_trait::async_trait]
impl PoWThreadStateSync for PoWStateManager {
    fn state(&self) -> PoWState {
        self.0.lock().unwrap().state.clone()
    }

    fn select(&self) -> Option<PoWThreadState> {
        self.0.lock().unwrap().select_thread()
    }

    fn sync(&self, state: &PoWThreadState, status: PowThreadStatus) -> bool {
        self.0.lock().unwrap().sync_thread(state, status)
    }
}
