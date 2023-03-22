use std::path::PathBuf;
use clap::{Arg, Command, value_parser};
use tide::{Request, Response};
use cyfs_debug::{CyfsLoggerBuilder, LogRecordMeta, PanicReportRequest, ReportLogItem};
use cyfs_util::get_service_data_dir;
use log::*;

const SERVICE_NAME: &str = "bug-server";

fn get_panic_file_path(req: &PanicReportRequest) -> PathBuf {
    // /cyfs/buf-server/panic/product_name/service_name/version/{exe_name}_{info_hash}.log
    let file_name = format!("{}_{}.log", &req.exe_name, &req.info.hash);
    let mut path = get_service_data_dir(SERVICE_NAME);
    path.push("panic");
    path.push(&req.product_name);
    path.push(&req.service_name);
    path.push(&req.version);
    let _ = std::fs::create_dir_all(&path);
    path.push(file_name);
    path
}

#[async_std::main]
async fn main() {
    CyfsLoggerBuilder::new_app("http-bug-server")
        .level("info")
        .console("info")
        .build()
        .unwrap()
        .start();

    let app = Command::new(SERVICE_NAME).version(cyfs_base::get_version())
        .arg(Arg::new("port").default_value("9550").value_parser(value_parser!(u16)))
        .get_matches();

    let port: u16 = *app.get_one("port").unwrap();

    let mut app = tide::new();

    app.at("/panic/:hash").post(move |mut req: Request<()>| {
        async move {
            let info = req.body_json::<PanicReportRequest>().await?;

            let path = get_panic_file_path(&info);

            info!("recv panic: service {}, version {}, hash {}, save to {}", info.service_name, info.version, info.info.hash, path.display());
            let msg = serde_json::to_string_pretty(&info.info).unwrap_or_else(|e| {
                let msg = format!("encode panic info to string error: {:?}, {}", &info.info, e);
                error!("{}", msg);
                msg
            }).replace("\\n", "\n");
            let content = format!(
                "CYFS service panic report: \nproduct:{}\nservice:{}\nbin:{}\nchannel:{}\ntarget:{}\nversion:{}\nmsg:{}",
                info.product_name,
                info.service_name,
                info.exe_name,
                info.channel,
                info.target,
                info.version,
                msg,
            );

            std::fs::write(path, content)?;

            Ok(Response::new(tide::StatusCode::Ok))
        }
    });

    let processor = cyfs_debug::HttpLogProcessor::new(vec![], move |meta: LogRecordMeta, list: Vec<ReportLogItem>| {
        async move {
            println!("recv logs: {:?}", meta);
            for ReportLogItem { index, record } in list {
                println!("recv log item: {}, {}", index, record);
            }

            Ok(())
        }
    });
    processor.register(&mut app);

    let address = format!("0.0.0.0:{}", port);
    info!("server will listen at {}", &address);
    app.listen(address).await.unwrap()
}
