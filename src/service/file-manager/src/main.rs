#![windows_subsystem = "windows"]

use crate::file_manager::{FileManager};
use tide::{Request, Response, StatusCode};
use cyfs_base::{BuckyResult, FILE_MANAGER_NAME, FILE_MANAGER_PORT, AnyNamedObject, RawFrom, ObjectId, RawConvertTo, AccessString};
use std::str::FromStr;
use tide::prelude::*;
use cyfs_core::get_system_dec_app;
use cyfs_lib::{NONGetObjectRequest, NONPutObjectRequest, SharedCyfsStack};

mod file_manager;
mod gateway_helper;

#[macro_use]
extern crate log;

#[derive(Deserialize)]
struct GetParam {
    fileid: String
}

async fn decode_desc_from_req(req: &mut Request<SharedCyfsStack>) -> BuckyResult<AnyNamedObject> {
    let desc_buf = req.body_bytes().await?;
    Ok(AnyNamedObject::clone_from_slice(&desc_buf)?)
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    cyfs_util::process::check_cmd_and_exec(FILE_MANAGER_NAME);
    
    cyfs_debug::CyfsLoggerBuilder::new_service(FILE_MANAGER_NAME)
        .level("info")
        .console("info")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-service", FILE_MANAGER_NAME)
        .build()
        .start();

    let stack = SharedCyfsStack::open_default(Some(get_system_dec_app().clone())).await.map_err(|e| {
        error!("open shared stack err {}", e);
        e
    })?;
    
    let database = cyfs_util::get_cyfs_root_path().join("data").join(FILE_MANAGER_NAME).join("file.sqlite");
    if database.exists() {
        info!("find old file-manager database {}, merge to cyfs stack", database.display());
        if let Err(e) = FileManager::merge(&database, stack.clone()).await {
            error!("merge old database to stack err {}, try re-merge at next startup", e);
        }
    }

    gateway_helper::register();

    let mut app = tide::with_state(stack.clone());

    app.at("/get_file").get(move |req: Request<SharedCyfsStack>| async move {
        let param = req.query::<GetParam>()?;
        if let Ok(file_id) = ObjectId::from_str(&param.fileid) {
            match req.state().non_service().get_object(NONGetObjectRequest::new_noc(file_id, None)).await {
                Ok(file_desc) => {
                    let mut resp = Response::new(StatusCode::Ok);
                    resp.set_body(file_desc.object.object_raw);
                    return Ok(resp);
                }
                Err(e) => {
                    error!("get file {} desc error: {}", file_id, e.to_string());
                }
            }
        } else {
            error!("invaild fileid {}", param.fileid);
        }

        Ok(Response::new(StatusCode::BadRequest))
    });

    app.at("/set_file")
        .post(move |mut req: Request<SharedCyfsStack>| async move {
            let desc_buf = req.body_bytes().await?;
            let desc = AnyNamedObject::clone_from_slice(&desc_buf)?;
            let id = desc.calculate_id();
            let mut request = NONPutObjectRequest::new_noc(id.clone(), desc.to_vec().unwrap());
            request.access = Some(AccessString::full());
            match req.state().non_service().put_object(request).await {
                Ok(resp) => {
                    info!("set desc {} success, resp {}", &id, resp.result.to_string());
                    return Ok(Response::new(StatusCode::Ok));
                }
                Err(e) => {
                    error!("set desc {} failed, err {}", id, e);
                }
            }

            Ok(Response::new(StatusCode::BadRequest))
        });

    let addr = format!("127.0.0.1:{}", FILE_MANAGER_PORT);
    app.listen(addr).await?;

    Ok(())
}
