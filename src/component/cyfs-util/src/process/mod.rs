mod pid_lock;

#[cfg(windows)]
mod win_process;

mod cmd_exec;
mod process_lock;
mod process_mutex;

mod daemon;

//pub(crate) use process_lock::ProcessLock;
pub use process_mutex::ProcessMutex;

pub use cmd_exec::{
    check_cmd_and_exec, check_cmd_and_exec_ext, check_cmd_and_exec_with_args, check_cmd_and_exec_with_args_ext, check_process_mutex,
    check_process_status, prepare_args, try_enter_proc, try_stop_process, ProcessAction,
    ProcessStatusCode,
};

pub use daemon::daemon::launch_as_daemon;
