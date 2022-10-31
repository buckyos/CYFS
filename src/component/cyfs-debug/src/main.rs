use log::*;

use cyfs_debug::*;

async fn main_run() {
    
    CyfsLoggerBuilder::new_app("cyfs-debug")
        .level("trace")
        .console("trace")
        .enable_bdt(Some("warn"), Some("warn"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "cyfs-debug")
        .exit_on_panic(true)
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    debug!("output debug log");
    info!("output info log");
    warn!("output warn log");
    error!("output error log");

    async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}

fn main() {
    crate::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}