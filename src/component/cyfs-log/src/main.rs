use log::*;

#[async_std::main]
async fn main() {
    
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

    
    debug!("output debug log");
    info!("output info log");
    warn!("output warn log");
    error!("output error log");

    async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}