use dsg_test::non_witness::AllInOneMiner;


#[async_std::main]
async fn main() {
    cyfs_debug::CyfsLoggerBuilder::new_app("dsg-all-in-one")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("off"), Some("off"))
        .module("cyfs-lib", Some("off"), Some("off"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("dsg-all-in-one", "dsg-all-in-one")
        .build()
        .start();

    let _ = AllInOneMiner::new().await.unwrap();

    async_std::task::block_on(async_std::future::pending::<()>());
}
