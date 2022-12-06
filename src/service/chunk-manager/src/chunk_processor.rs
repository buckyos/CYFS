use tide::{Request, Response, StatusCode};

use cyfs_base::*;

use crate::chunk_manager::{ChunkManager};
use crate::chunk_delegate;
// use crate::chunk_tx;
use crate::chunk_context::ChunkContext;
use std::io::Write;

async fn get_chunk_data(trace:&str, chunk_manager: &ChunkManager, chunk_get_req:&cyfs_chunk::ChunkGetReq, _source_device_id:&DeviceId)->BuckyResult<Response>{
    // if &chunk_get_req.client_device_id!=source_device_id{
    //     error!("{} client peer id is not the data owner, will not resp raw data, chunk id:{}", trace, chunk_get_req.chunk_id);
    //     return Err(BuckyError::from(BuckyErrorCode::PermissionDenied));
    // }

    info!("{} client peer id is the data owner, and request raw data, just return, chunk id:{}", trace, chunk_get_req.chunk_id());
    let reader = chunk_manager.get(chunk_get_req.chunk_id()).await?;

    /*
    let mut chunk_data = Vec::new();
    let _ = read.read_to_end(&mut chunk_data).await.map_err(|e|{
        error!("{} read chunk data failed, msg:{}", trace, e.to_string());
        BuckyError::from(e)
    })?;

    // verify chunk id
    let actual_id = ChunkId::calculate_sync(&chunk_data)?;
    if &actual_id != chunk_get_req.chunk_id() {
        error!("local chunk id mismatch! actual {}, except {}", &actual_id, chunk_get_req.chunk_id());
        chunk_manager.delete(chunk_get_req.chunk_id())?;
        return Err(BuckyError::from(BuckyErrorCode::NotMatch));
    }
    */

    info!("{} get chunk data success, resp", trace);
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(http_types::Body::from_reader(reader, None));

    info!("{} OK", trace);
    return Ok(resp);
}

async fn get_chunk_data_with_meta(trace:&str, chunk_manager: &ChunkManager, chunk_get_req:&cyfs_chunk::ChunkGetReq, source_peer_sec:&PrivateKey, source_device_id:&DeviceId)->BuckyResult<Response>{
    // if &chunk_get_req.client_device_id!=source_device_id{
    //     error!("{} client peer id is not the data owner, will not resp raw data with meta, chunk id:{}", trace, chunk_get_req.chunk_id);
    //     return Err(BuckyError::from(BuckyErrorCode::PermissionDenied));
    // }

    info!("{} client peer id is the data owner, and request raw data with meta, just return, chunk id:{}", trace, chunk_get_req.chunk_id());
    let chunk_data = chunk_manager.get_data(chunk_get_req.chunk_id()).await?;

    info!("{} get chunk data with meta success, resp", trace);
    let chunk_get_resp = cyfs_chunk::ChunkGetResp::new_raw(
        &source_peer_sec,
        &source_device_id,
        &chunk_get_req.client_device_id(),
        &chunk_get_req.chunk_id(),
        chunk_data
    )?;
    let resp_str = chunk_get_resp.to_vec()?;
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(resp_str);

    info!("{} OK", trace);
    return Ok(resp);
}

async fn get_chunk_redirect(trace:&str, _chunk_manager: &ChunkManager, chunk_get_req:&cyfs_chunk::ChunkGetReq, delegate:&chunk_delegate::ChunkDelegate, source_peer_sec:&PrivateKey, source_device_id:&DeviceId)->BuckyResult<Response>{
    if delegate.price != * chunk_get_req.price() {
        error!("{} request price not match, request:{}, require:{}", trace, chunk_get_req.price(), delegate.price);
        return Err(BuckyError::from("invalid param"));
    }

    info!("{} get chunk data should redirect, resp", trace);
    let chunk_get_resp = cyfs_chunk::ChunkGetResp::new_redirect(
        &source_peer_sec,
        &source_device_id,
        &delegate.miner_device_id,
        chunk_get_req.client_device_id(),
        chunk_get_req.chunk_id(),
        &delegate.price,
    )?;
    let resp_str = chunk_get_resp.to_vec()?;
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(resp_str);

    info!("{} OK", trace);
    return Ok(resp);
}

pub async fn get_chunk(ctx: ChunkContext, req: & mut Request<()>) ->BuckyResult<Response>{
    info!("[get_chunk] init chunk_manager");
    let chunk_manager = ChunkManager::new(&ctx);
    
    let source_peer_sec = chunk_manager.get_private_key();
    let source_device_id = chunk_manager.get_device_id();

    info!("[get_chunk] receive req");
    let body = req.body_bytes().await?;
    let chunk_get_req = cyfs_chunk::ChunkGetReq::clone_from_slice(&body)?;
    // if chunk_get_req.source_device_id()!=&source_device_id {
    //     error!("[get_chunk] receive req failed, chunk_manager.source_device_id:{}, chunk_get_req.source_device_id:{}", 
    //         source_device_id, 
    //         chunk_get_req.source_device_id());
    //     return Err(BuckyError::from("invalid param"));
    // }

    let trace = format!("[get_chunk] [{}]", chunk_get_req.chunk_id());

    info!("{} verify req(defualt ignore)", trace);
    // let client_public_key = chunk_manager.get_peer_public_key(&chunk_get_req.client_device_id).await?;
    // if !chunk_get_req.verify(&client_public_key) {
    //     error!("{} verify req failed", trace);
    //     return Err(BuckyError::from("invalid param"));
    // }

    // different get strategy
    match chunk_get_req.req_type() {
        cyfs_chunk::ChunkGetReqType::Data=>{
            info!("{} get_chunk_data", trace);
            get_chunk_data(&trace, &chunk_manager, &chunk_get_req, &source_device_id).await
        },
        cyfs_chunk::ChunkGetReqType::DataWithMeta=>{
            info!("{} get_chunk_data_with_meta", trace);
            get_chunk_data_with_meta(&trace, &chunk_manager, &chunk_get_req, source_peer_sec, &source_device_id).await
        },
        _=>{

            let delegate_ret = chunk_delegate::find_delegate(chunk_get_req.chunk_id()).await;
            match delegate_ret {
                Ok(delegate)=>{
                    info!("{} get_chunk_redirect", trace);
                    get_chunk_redirect(&trace, &chunk_manager, &chunk_get_req, &delegate, source_peer_sec, &source_device_id).await
                },
                _=>{
                    info!("{} get_chunk_data_with_meta", trace);
                    get_chunk_data_with_meta(&trace, &chunk_manager, &chunk_get_req, source_peer_sec, &source_device_id).await
                }
            }
        }
    }
}

pub async fn set_chunk(ctx: ChunkContext, req: & mut Request<()>) ->BuckyResult<Response>{

    info!("[set_chunk] init chunk_manager");
    let chunk_manager = ChunkManager::new(&ctx);
    
    let source_peer_sec = chunk_manager.get_private_key();
    let source_public_key = chunk_manager.get_public_key();
    let source_device_id = chunk_manager.get_device_id();

    info!("[set_chunk] receive chunk set request");
    let body = req.body_bytes().await?;
    let chunk_set_req = cyfs_chunk::ChunkSetReq::clone_from_slice(&body)?;
    if chunk_set_req.source_device_id()!= &source_device_id {
        error!("[set_chunk] receive chunk set request DeviceId mismatch! req {}, this {}", chunk_set_req.source_device_id(), source_device_id);
        return Err(BuckyError::from("invalid param"));
    }

    let trace = format!("[set_chunk] [{}]", chunk_set_req.chunk_id().to_string());

    info!("{} verify chunk set request", trace);
    if !chunk_set_req.verify(source_public_key) {
        error!("{} verify chunk set request failed!", trace);
        return Err(BuckyError::from("invalid param"));
    }

    let chunk_id = ChunkId::calculate(chunk_set_req.data()).await?;
    if chunk_set_req.chunk_id() != &chunk_id {
        error!("{} verify ChunkId failed! except {}, actual {}", trace, chunk_set_req.chunk_id(), chunk_id);
        {
            let err_path = std::path::PathBuf::from("/cyfs/data/chunk_manager/err_chunk");
            if let Err(e) = std::fs::create_dir_all(&err_path) {
                error!("create dir error! {}, {}", err_path.display(), e);
            }

            let mut file = std::fs::File::create(err_path.join(format!("{}.chunk", chunk_id))).unwrap();
            if let Err(e) = file.write_all(chunk_set_req.data()) {
                error!("write error chunk failed! {}, {}", err_path.display(), e);
            }
            if let Err(e) = file.flush() {
                error!("flush error chunk failed! {}, {}", err_path.display(), e);
            }
        }
        return Err(BuckyError::from("invalid param"));
    }

    info!("{} save chunk data", trace);
    let _ = chunk_manager.set(&chunk_id, chunk_set_req.data()).await.map_err(|e|{
        error!("~>set_chunk, {}, save chunk error: {}", trace, e.to_string());
        BuckyError::from(e)
    })?;

    info!("{} set chunk success, sign and resp", trace);
    let chunk_set_resp = cyfs_chunk::ChunkSetResp::sign(
        &source_peer_sec,
        &source_device_id,
        &chunk_id,
        cyfs_chunk::ChunkSetStatus::Ok
    )?;

    let resp_str = chunk_set_resp.to_vec()?;
    let mut resp = Response::new(StatusCode::Ok);
    resp.set_body(resp_str);

    info!("{} OK", trace);

    Ok(resp)
}

pub async fn create_chunk_delegate(_ctx: ChunkContext, _req: & mut Request<()>) ->BuckyResult<Response>{

    info!("[create_chunk_delegate] init chunk_manager");
    // let mut chunk_manager = ChunkManager::new(&ctx);
    // let source_peer_sec = chunk_manager.get_private_key();
    // let source_public_key = chunk_manager.get_public_key();
    // let source_device_id_obj = chunk_manager.get_device_id();
    // let source_device_id = source_device_id_obj;

    // info!("[create_chunk_delegate] receive request");
    // let body = req.body_bytes().await?;
    // let chunk_create_delegate_req: cyfs_chunk::ChunkCreateDelegateReq = cyfs_chunk::Deserializer::from_vec(&body)?;
    // if chunk_create_delegate_req.source_device_id!=source_device_id {
    //     return Err(BuckyError::from("invalid param"));
    // }

    // let trace = format!("[create_chunk_delegate] [{}]", chunk_create_delegate_req.chunk_id.to_string());

    // info!("{} verify request", trace);
    // if !chunk_create_delegate_req.verify(source_public_key) {
    //     return Err(BuckyError::from("invalid param"));
    // }

    // info!("{} add_chunk_delegate meta", trace);
    // let _ = chunk_delegate::add_chunk_delegate(
    //     &chunk_create_delegate_req.miner_device_id,
    //     &chunk_create_delegate_req.chunk_id,
    //     &chunk_create_delegate_req.price
    // ).await?;

    // info!("{} create_union_account", trace);
    // let tx_hash:TxHash = chunk_manager.create_union_account(&chunk_create_delegate_req.miner_device_id, &chunk_create_delegate_req.balance).await?;

    // info!("{} success, resp", trace);
    // TODO:
    // let chunk_create_delegate_resp = cyfs_chunk::ChunkCreateDelegateResp::sign(
    //     &source_peer_sec,
    //     &source_device_id,
    //     &chunk_create_delegate_req.miner_device_id,
    //     &chunk_create_delegate_req.chunk_id,
    //     &chunk_create_delegate_req.price,
    //     cyfs_chunk::ChunkCreateDelegateStatus::Delegated,
    //     tx_hash
    // )?;
    // let resp_data = cyfs_chunk::Serializer::to_vec(&chunk_create_delegate_resp)?;

    let resp = Response::new(StatusCode::Ok);
    // resp.set_body(resp_str);

    // info!("{} OK", trace);
    Ok(resp)
}

pub async fn query_chunk_delegate(_ctx: ChunkContext, _req: & mut Request<()>) ->BuckyResult<Response>{
    // info!("[query_chunk_delegate] init chunk_manager");
    // let mut chunk_manager = ChunkManager::new(&ctx);
    // let source_peer_sec = chunk_manager.get_private_key();
    // let source_public_key = chunk_manager.get_public_key();
    // let source_device_id_obj = chunk_manager.get_device_id();
    // let source_device_id = source_device_id_obj;

    // info!("[query_chunk_delegate] receive request");
    // let body = req.body_bytes().await?;
    // let chunk_create_delegate_query_req: cyfs_chunk::ChunkCreateDelegateQueryReq = cyfs_chunk::Deserializer::from_vec(&body)?;
    // if chunk_create_delegate_query_req.source_device_id!=source_device_id {
    //     return Err(BuckyError::from("invalid param"));
    // }

    // let trace = format!("[query_chunk_delegate] [{}]", chunk_create_delegate_query_req.chunk_id.to_string());

    // info!("{} verify request", trace);
    // if !chunk_create_delegate_query_req.verify(source_public_key) {
    //     return Err(BuckyError::from("invalid param"));
    // }

    // let d = chunk_delegate::find_delegate(&chunk_create_delegate_query_req.chunk_id).await?;

    // let chunk_create_delegate_query_resp = cyfs_chunk::ChunkCreateDelegateQueryResp::sign(
    //     source_peer_sec,
    //     &source_device_id,
    //     &chunk_create_delegate_query_req.miner_device_id,
    //     &chunk_create_delegate_query_req.chunk_id,
    //     d.state.to(),
    //     0,
    // )?;

    // let resp_str = cyfs_chunk::Serializer::to_string(&chunk_create_delegate_query_resp)?;
    let resp = Response::new(StatusCode::Ok);
    // resp.set_body(resp_str);

    // info!("{} OK", trace);
    Ok(resp)
}

pub async fn redeem_chunk_proof(_ctx: ChunkContext, _req: & mut Request<()>) ->BuckyResult<Response>{
    // info!("[redeem_chunk_proof] init chunk manager");
    // let mut chunk_manager = ChunkManager::new(&ctx);
    // let source_peer_sec = chunk_manager.get_private_key();
    // let source_device_id_obj = chunk_manager.get_device_id();
    // let source_device_id = source_device_id_obj;

    // info!("[redeem_chunk_proof] receive req");
    // let body = req.body_bytes().await?;
    // let chunk_redeem_req: cyfs_chunk::ChunkRedeemReq = cyfs_chunk::Deserializer::from_vec(&body)?;
    // if chunk_redeem_req.source_device_id!=source_device_id {
    //     return Err(BuckyError::from("invalid param"));
    // }

    // let trace = format!("[redeem_chunk_proof] [{}]", chunk_redeem_req.chunk_id.to_string());

    // info!("{} verify req", trace);
    // let miner_public_key = chunk_manager.get_peer_public_key(&chunk_redeem_req.miner_device_id).await?;
    // let client_public_key = chunk_manager.get_peer_public_key(&chunk_redeem_req.client_device_id).await?;
    // if !chunk_redeem_req.verify(&miner_public_key, &client_public_key) {
    //     error!("{} verify req failed", trace);
    //     return Err(BuckyError::from("invalid param"));
    // }

    // info!("{} fetch_chunk_price and create a signed union transaction", trace);
    // let price = chunk_delegate::fetch_chunk_price(&chunk_redeem_req.miner_device_id, &chunk_redeem_req.chunk_id).await?;
    // let signed_tx:SignedTx = chunk_tx::add_chunk_tx(
    //     &chunk_manager,
    //     &chunk_redeem_req.miner_device_id,
    //     &chunk_redeem_req.client_device_id,
    //     &chunk_redeem_req.chunk_id,
    //     price
    // ).await?;

    // info!("{} sign resp", trace);
    // let chunk_redeem_resp = cyfs_chunk::ChunkRedeemResp::sign(
    //     &source_peer_sec,
    //     &source_device_id,
    //     &chunk_redeem_req.miner_device_id,
    //     &chunk_redeem_req.client_device_id,
    //     &chunk_redeem_req.chunk_id,
    //     signed_tx,
    // )?;

    // info!("{} success and resp", trace);
    // let resp_str = cyfs_chunk::Serializer::to_string(&chunk_redeem_resp)?;
    let resp = Response::new(StatusCode::Ok);
    // resp.set_body(resp_str);

    Ok(resp)
}
