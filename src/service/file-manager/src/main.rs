#![windows_subsystem = "windows"]

use crate::file_manager::FILE_MANAGER;
use std::borrow::Cow;
use tide::{Request, Response, StatusCode};
use cyfs_base::{BuckyResult, FILE_MANAGER_NAME, FILE_MANAGER_PORT, AnyNamedObject, RawFrom, ObjectId, RawConvertTo};
use std::str::FromStr;

mod file_manager;
mod gateway_helper;

#[macro_use]
extern crate log;

fn find_param_from_req<'a>(req: &'a Request<()>, param: &str) -> Option<Cow<'a, str>> {
    match req.url().query_pairs().find(|(x, _)| x == param) {
        None => {
            error!(
                "can`t find param {} from query {}",
                param,
                req.url().query().unwrap_or("NULL")
            );
            None
        }
        Some((_, v)) => Some(v),
    }
}

async fn decode_desc_from_req(req: &mut Request<()>) -> BuckyResult<AnyNamedObject> {
    let desc_buf = req.body_bytes().await?;
    Ok(AnyNamedObject::clone_from_slice(&desc_buf)?)
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    cyfs_util::process::check_cmd_and_exec(FILE_MANAGER_NAME);
    
    cyfs_debug::CyfsLoggerBuilder::new_service(FILE_MANAGER_NAME)
        .level("debug")
        .console("debug")
        .enable_bdt(Some("debug"), Some("debug"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", FILE_MANAGER_NAME)
        .build()
        .start();

    
    let mut database = cyfs_util::get_cyfs_root_path().join("data").join(FILE_MANAGER_NAME);
    let _ = std::fs::create_dir_all(&database).map_err(|e| {
        error!("create database dir {} failed.", database.display());
        e
    })?;
    database.push("file.sqlite");

    info!("database is:{}", database.display());

    let _ = FILE_MANAGER.lock().await.init(&database).map_err(|e| {
        error!("init file manager failed, msg:{}", e.to_string());
        std::io::Error::from(std::io::ErrorKind::Interrupted)
    })?;

    gateway_helper::register();

    let mut app = tide::new();

    app.at("/get_file").get(move |req: Request<()>| async move {
        loop {
            let fileid_ret = find_param_from_req(&req, "fileid");
            if fileid_ret.is_none() {
                break;
            }

            if let Ok(file_id) = ObjectId::from_str(fileid_ret.as_ref().unwrap()) {
                let file_manager = FILE_MANAGER.lock().await;

                match file_manager.get(&file_id).await {
                    Ok(file_desc) => {
                        let mut resp = Response::new(StatusCode::Ok);
                        match file_desc.to_vec() {
                            Ok(buf) => {
                                resp.set_body(buf);
                                return Ok(resp);
                            }
                            Err(e) => {
                                error!("encode file_desc err {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("get file {} desc error: {}", file_id, e.to_string());
                    }
                }
            } else {
                error!("invaild fileid {}", fileid_ret.as_ref().unwrap());
            }

            break;
        }

        Ok(Response::new(StatusCode::BadRequest))
    });

    app.at("/set_file")
        .post(move |mut req: Request<()>| async move {
            match decode_desc_from_req(&mut req).await {
                Ok(desc) => {
                    let id = desc.calculate_id();
                    let file_manager = FILE_MANAGER.lock().await;
                    match file_manager.set(&id, &desc).await {
                        Ok(_) => {
                            info!("set desc {} success", &id);
                            return Ok(Response::new(StatusCode::Ok));
                        }
                        Err(e) => {
                            error!("set desc {} failed, err {}", id, e);
                        }
                    }
                }
                Err(e) => {
                    error!("decode filedesc error: {}", e);
                }
            }

            Ok(Response::new(StatusCode::BadRequest))
        });

    let addr = format!("127.0.0.1:{}", FILE_MANAGER_PORT);
    app.listen(addr).await?;

    Ok(())
}
