mod pid_lock;

#[cfg(windows)]
mod win_process;

mod cmd_exec;
mod process_lock;
mod process_mutex;

mod daemon;

//pub(crate) use process_lock::ProcessLock;
pub use process_mutex::ProcessMutex;

pub use cmd_exec::*;
pub use daemon::daemon::launch_as_daemon;
