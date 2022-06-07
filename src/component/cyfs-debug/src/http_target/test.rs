use cyfs_base::BuckyResult;

use crate::*;

pub struct LogServer {}

impl OnRecvLogRecords for LogServer {
    fn on_log_records(&self, meta: LogRecordMeta, list: Vec<ReportLogItem>) -> BuckyResult<()> {
        println!("recv logs: {:?}", meta);
        for ReportLogItem { index, record } in list {
            println!("recv log item: {}, {}", index, record);
        }

        Ok(())
    }
}

// 每个服务上报的关联信息，可动态扩展
const LOG_HEADER_MACHINE: &str = "machine";
const LOG_HEADER_SERVICE: &str = "service";

#[async_std::test]
async fn main() {
    let addr = "192.168.100.110:7878";

    // 首先启动一个log_server
    let log_server = LogServer {};
    let headers = vec![LOG_HEADER_MACHINE.to_owned(), LOG_HEADER_SERVICE.to_owned()];

    let server = HttpLogReceiver::new(addr, headers, Box::new(log_server));
    server.start();

    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    // let target = Box::new(ConsoleCyfsLogTarget {});
    let url = format!("http://{}/logs", addr);
    let url = url::Url::parse(&url).unwrap();

    let headers = vec![
        (LOG_HEADER_MACHINE.to_owned(), "test1".to_owned()),
        (LOG_HEADER_SERVICE.to_owned(), "chunk-node".to_owned()),
    ];
    let reporter = HttpLogReporter::new(url, headers, 0);
    reporter.start();
    let target = Box::new(reporter);

    CyfsLoggerBuilder::new_app("cyfs-debug")
        .level("trace")
        .console("trace")
        .enable_bdt(Some("warn"), Some("warn"))
        .target(target)
        .disable_module(vec!["surf"], LogLevel::Warn)
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "cyfs-debug")
        .exit_on_panic(true)
        .build()
        .start();

    loop {
        debug!("output debug log");
        info!("output info log");
        warn!("output warn log");
        error!("output error log");

        async_std::task::sleep(std::time::Duration::from_secs(1)).await;
    }
    

    // async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}
