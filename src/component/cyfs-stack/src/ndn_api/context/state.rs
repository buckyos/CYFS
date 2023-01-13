use cyfs_base::*;
use cyfs_bdt::channel::DownloadSessionState;
use cyfs_bdt::ndn::channel::DownloadSession;
use cyfs_bdt::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NDNTaskCancelStrategy {
    AutoCancel, // task auto finished when all source finished
    WaitingSource,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct ContextSourceIndex {
    source: DeviceId,
    chunk: ChunkId,
}

// use cyfs_bdt::
struct ContextSourceDownloadState {
    update_at: Timestamp,
    state: Option<DownloadSessionState>,
}

#[derive(Clone)]
pub(super) struct ContextSourceDownloadStateManager {
    all: Arc<Mutex<HashMap<ContextSourceIndex, ContextSourceDownloadState>>>,
    cancel_strategy: NDNTaskCancelStrategy,
}

impl ContextSourceDownloadStateManager {
    pub fn new(cancel_strategy: NDNTaskCancelStrategy) -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
            cancel_strategy,
        }
    }

    pub fn on_new_session(
        &self,
        task: &dyn LeafDownloadTask,
        session: &DownloadSession,
        update_at: Timestamp,
    ) {
        self.add_session(task, session, update_at);

        let this = self.clone();
        let session = session.clone();
        let group = task.abs_group_path();
        async_std::task::spawn(async move {
            let state = session.wait_finish().await;
            this.on_session_finished(&session, state, group);
        });
    }

    fn on_session_finished(
        &self,
        session: &DownloadSession,
        state: DownloadSessionState,
        group: Option<String>,
    ) {
        let source = session.source();
        info!(
            "task session on source finished! task={:?}, source={}, chunk={}, session={:?}, state={:?}",
            group,
            source.target,
            session.chunk(),
            session.session_id(),
            state,
        );

        let index = ContextSourceIndex {
            source: source.target.clone(),
            chunk: session.chunk().clone(),
        };

        let mut all = self.all.lock().unwrap();
        let ret = all.get_mut(&index);
        if ret.is_none() {
            error!("task session finished but not found in state manager! task={:?}, source={}, chunk={}, session={:?}", 
                group, source.target, index.chunk, session.session_id());
            return;
        }

        let item = ret.unwrap();
        if item.state.is_some() {
            error!("task session on source already finished! task={:?}, source={}, session={:?}, current state={:?}",
                group,
                source.target,
                session.session_id(),
                item.state.as_ref().unwrap(),
            );
        }

        item.state = Some(state);
    }

    pub fn on_drain(&self, task: &dyn LeafDownloadTask, when: Timestamp) {
        match self.cancel_strategy {
            NDNTaskCancelStrategy::AutoCancel => {
                info!(
                    "task drain source, now will try check and cancel task! task={:?}, when={}",
                    task.abs_group_path(),
                    when
                );
                self.check_and_cancel_task(task);
            }
            NDNTaskCancelStrategy::WaitingSource => {}
        }
    }

    fn check_and_cancel_task(&self, task: &dyn LeafDownloadTask) {
        let all = self.all.lock().unwrap();

        let mut err = None;
        let mut downloading = false;
        for (index, item) in &*all {
            if item.state.is_none() {
                warn!(
                    "task drain source but still not finishe or canceled! task={:?}, source={}, chunk={}",
                    task.abs_group_path(),
                    index.source,
                    index.chunk,
                );
                downloading = true;
                break;
            }

            match item.state.as_ref().unwrap() {
                DownloadSessionState::Downloading => {
                    error!(
                        "task drain source but still in downloading state! task={:?}, source={}, chunk={}",
                        task.abs_group_path(),
                        index.source,
                        index.chunk,
                    );
                    downloading = true;
                    break;
                }
                DownloadSessionState::Finished => {}
                DownloadSessionState::Canceled(e) => {
                    err = Some(e.clone());
                }
            }
        }

        drop(all);

        if downloading {
            return;
        }

        warn!(
            "task all source finished or canceled! task={:?}, err={:?}",
            task.abs_group_path(),
            err,
        );

        if let Some(e) = err {
            warn!(
                "will cancel task on source error! task={:?}, err={}",
                task.abs_group_path(),
                e
            );
            match task.cancel_by_error(e) {
                Ok(state) => {
                    info!(
                        "cancel task on source error complete! task={:?}, state={:?}",
                        task.abs_group_path(),
                        state
                    );
                }
                Err(e) => {
                    error!(
                        "cancel task on source error failed! task={:?}, {}",
                        task.abs_group_path(),
                        e
                    );
                }
            }
        }
    }

    fn add_session(&self, task: &dyn LeafDownloadTask, session: &DownloadSession, update_at: Timestamp,) {
        let source = session.source();

        let index = ContextSourceIndex {
            source: source.target.clone(),
            chunk: session.chunk().clone(),
        };

        let mut all = self.all.lock().unwrap();
        match all.entry(index) {
            Entry::Vacant(v) => {
                let item = ContextSourceDownloadState { state: None, update_at };

                info!(
                    "new task session on source: task={:?}, source={}, chunk={}, session={:?}, update_at={}",
                    task.abs_group_path(),
                    source.target,
                    session.chunk(),
                    session.session_id(),
                    item.update_at,
                );
                v.insert(item);
            }
            Entry::Occupied(mut o) => {
                warn!(
                    "task session on same source! task={:?}, source={}, chunk={}, session={:?}, prev update_at={}, new update_at={}",
                    task.abs_group_path(),
                    source.target,
                    session.chunk(),
                    session.session_id(),
                    o.get().update_at,
                    update_at,
                );
                o.get_mut().state = None;
                o.get_mut().update_at = update_at;
            }
        }
    }
}
