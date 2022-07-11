
mod service;
mod perf_db;
mod storage;
mod config;
mod perf_manager;

use service::*;
use cyfs_util::process::ProcessAction;
use cyfs_lib::SharedCyfsStack;
use async_std::sync::Arc;


#[async_std::main]
async fn main() {
    let status = cyfs_util::process::check_cmd_and_exec("perf-service");
    if status == ProcessAction::Install {
        // 这里可以做一些初始化工作, App安装/更新/重新安装后一定会触发一次

        // 注意，install的时候不会上进程锁，也就不能通过stop和status命令正确的检查状态。这里install完成后应该退出了
        std::process::exit(0);
    }

    cyfs_debug::CyfsLoggerBuilder::new_service("perf-service")
    .level("debug")
    .console("debug")
    .enable_bdt(Some("trace"), Some("trace"))
    .build()
    .unwrap()
    .start();

    // 使用默认配置初始化non-stack，因为是跑在gateway后面，共享了gateway的协议栈，所以配置使用默认即可
    // TODO: 这里需要用perf app的app id来初始化SharedObjectStack
    let cyfs_stack = SharedCyfsStack::open_default(None).await.unwrap();
    let _ = cyfs_stack.online().await;

    log::info!("perf-service run as {}", cyfs_stack.local_device_id());
    let mut service = PerfService::new(cyfs_stack, false);
    service.init().await;

    PerfService::start(Arc::new(service));

    async_std::task::block_on(async_std::future::pending::<()>());
}
