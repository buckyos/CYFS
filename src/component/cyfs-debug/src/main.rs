use log::*;
use http_types::Url;

use cyfs_debug::*;
use crate::HttpLogReporter;

async fn main_run() {
    let http_reporter = HttpLogReporter::new(Url::parse("http://127.0.0.1:9550/logs").unwrap(), vec![], 100);
    http_reporter.start();
    CyfsLoggerBuilder::new_app("cyfs-debug")
        .level("trace")
        .console("trace")
        .enable_bdt(Some("warn"), Some("warn"))
        .target(Box::new(http_reporter))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "cyfs-debug")
        .exit_on_panic(true)
        .http_bug_report("http://127.0.0.1:9550/panic/")
        .build()
        .start();

    cyfs_debug::ProcessDeadHelper::instance().enable_exit_on_task_system_dead(None);

    debug!("output debug log");
    info!("output info log");
    warn!("output warn log");
    error!("output error log");

    
    async_std::task::spawn(async move {
        async_std::task::sleep(std::time::Duration::from_secs(30)).await;
        unreachable!("test cyfs-debug panic");
    });

    // async_std::task::sleep(std::time::Duration::from_secs(1000)).await;

    // info!("create minidump file");
    // let helper = cyfs_debug::DumpHelper::get_instance();
    // helper.dump();

    async_std::task::sleep(std::time::Duration::from_secs(1000)).await;

    // cyfs_debug::create_dump(Path::new("."), "minidump_%p.dmp", false);
}

fn main() {
    crate::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}
