use cyfs_base::*;
use cyfs_bdt::channel::DownloadSessionState;
use cyfs_bdt::ndn::channel::DownloadSession;
use cyfs_bdt::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NDNTaskCancelStrategy {
    AutoCancel, // task auto finished when all source finished
    WaitingSource,
}


#[derive(Clone, Debug)]
pub(crate) enum NDNTaskCancelSourceStrategy {
    None, 
    ZeroSpeed(Duration, Duration)
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
    cancel_source_strategy: NDNTaskCancelSourceStrategy
}

impl ContextSourceDownloadStateManager {
    pub fn new(
        cancel_strategy: NDNTaskCancelStrategy, 
        cancel_source_strategy: NDNTaskCancelSourceStrategy
    ) -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
            cancel_strategy,
            cancel_source_strategy
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
            let state =  match this.cancel_source_strategy {
                NDNTaskCancelSourceStrategy::ZeroSpeed(atomic, timeout) => {
                    {
                        let session = session.clone();
                        let group = group.clone();
                        async_std::task::spawn(async move {
                            let mut zero_speed_time = Duration::from_secs(0);
                            loop {
                                if async_std::future::timeout(atomic, session.wait_finish()).await.is_ok() {
                                    break;
                                }
                                if session.cur_speed() == 0 {
                                    zero_speed_time += atomic;
                                    info!(
                                        "task session running but no speed, task={:?}, chunk={}, session={:?}, duration={:?}",
                                        group,
                                        session.chunk(),
                                        session.session_id(),
                                        zero_speed_time
                                    );
                                    if zero_speed_time > timeout {
                                        error!(
                                            "task session zero speed for long time cancel it, task={:?}, chunk={}, session={:?}",
                                            group,
                                            session.chunk(),
                                            session.session_id(),
                                        );
                                        session.cancel_by_error(BuckyError::new(BuckyErrorCode::Timeout, "zero speed"));
                                        break;
                                    }
                                } else {
                                    zero_speed_time = Duration::from_secs(0);
                                }
                            }
                        });
                    }
                    session.wait_finish().await
                }, 
                NDNTaskCancelSourceStrategy::None => session.wait_finish().await
            };
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
            NDNTaskCancelStrategy::WaitingSource => {
                debug!(
                    "task drain source and will still waiting for new source! task={:?}, when={}",
                    task.abs_group_path(),
                    when
                );
            }
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

        if let Some(err) = &err {
            warn!(
                "task all source finished or canceled! task={:?}, err={}",
                task.abs_group_path(),
                err,
            );
        } else {
            info!(
                "task all source finished! task={:?}",
                task.abs_group_path(),
            );
        }
        

        if let Some(e) = err {
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
