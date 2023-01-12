use cyfs_base::*;
use cyfs_bdt::channel::DownloadSessionState;
use cyfs_bdt::ndn::channel::DownloadSession;
use cyfs_bdt::*;

use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NDNTaskCancelStrategy {
    AutoCancel, // task auto finished when all source finished
    WaitingSource,
}

// use cyfs_bdt::
struct ContextSourceDownloadState {
    source: DeviceId,
    state: Option<DownloadSessionState>,
}

#[derive(Clone)]
pub(super) struct ContextSourceDownloadStateManager {
    all: Arc<Mutex<Vec<ContextSourceDownloadState>>>,
    cancel_strategy: NDNTaskCancelStrategy,
}

impl ContextSourceDownloadStateManager {
    pub fn new(cancel_strategy: NDNTaskCancelStrategy) -> Self {
        Self {
            all: Arc::new(Mutex::new(vec![])),
            cancel_strategy,
        }
    }

    pub fn on_new_session(&self, task: &dyn LeafDownloadTask, session: &DownloadSession) {
        self.add_session(task, session);

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
            "task session on source finished! task={:?}, source={}, session={:?}",
            group,
            source.target,
            session.session_id()
        );

        let mut all = self.all.lock().unwrap();
        let ret = all.iter_mut().find(|item| item.source == source.target);
        if ret.is_none() {
            error!("task session finished but not found in state manager! task={:?}, source={}, session={:?}", group, source.target, session.session_id());
            return;
        }

        let item = ret.unwrap();
        assert!(item.state.is_none());
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
        for item in &*all {
            if item.state.is_none() {
                warn!(
                    "task drain source but still not finishe or canceled! task={:?}, source={}",
                    task.abs_group_path(),
                    item.source
                );
                downloading = true;
                break;
            }

            match item.state.as_ref().unwrap() {
                DownloadSessionState::Downloading(_) => {
                    error!(
                        "task drain source but still in downloading state! task={:?}, source={}",
                        task.abs_group_path(),
                        item.source
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

    fn add_session(&self, task: &dyn LeafDownloadTask, session: &DownloadSession) {
        let source = session.source();

        let mut all = self.all.lock().unwrap();
        let mut find = false;
        for i in 0..all.len() {
            let item = &mut all[i];
            if item.source == source.target {
                warn!(
                    "task session on same source! task={:?}, source={}, prev={:?}, new={:?}",
                    task.abs_group_path(),
                    item.source,
                    session.session_id(),
                    session.session_id()
                );
                item.state = None;

                find = true;
                break;
            }
        }

        if !find {
            let item = ContextSourceDownloadState {
                source: source.target.clone(),
                state: None,
            };

            info!(
                "new task session on source: task={:?}, source={}, session={:?}",
                task.abs_group_path(),
                item.source,
                session.session_id()
            );
            all.push(item);
        }
    }
}
