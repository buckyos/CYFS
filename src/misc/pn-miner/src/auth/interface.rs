use std::str::FromStr;
use async_std::sync::Arc;
use serde::{
    Serializer, 
    Deserializer, 
    de::{self, Visitor}
};
use tide::{
    Request, 
    Response, 
    Body, 
    prelude::*
};
use cyfs_base::*;
use super::storage::Storage;


/*
内部开白名单（要限ip）
req
uri: /pn/test_rent
method: POST
body: {
    from: DeviceId, ood的device id
    id: DeviceId,   pn的device id
}

resp
body: {
    err: 0 成功/4 pn不存在
}
*/

struct ServerImpl {
    pn: DeviceId, 
    storage: Storage
}

#[derive(Clone)]
struct Server(Arc<ServerImpl>);

impl Server {
    fn new(pn: DeviceId, storage: Storage) -> Self {
        Self(Arc::new(ServerImpl {
            pn, 
            storage
        }))
    }

    fn storage(&self) -> &Storage {
        &self.0.storage
    }

    fn pn(&self) -> &DeviceId {
        &self.0.pn
    }
}

pub async fn listen(port: u16, pn: DeviceId, storage: Option<Storage>) -> BuckyResult<()> {
    if storage.is_none() {
        return Ok(());
    }
    let storage = storage.unwrap();

    let mut server = tide::with_state(Server::new(pn, storage));  

    /*
    获取PN列表
    req
    uri: /pn/list
    method: GET

    resp
    body: [{
        id: DeviceId, 
        bandwidth: 10M/20M, 带宽
        limit: Number, 总共多少 
        used: Number, 用了多少
    },]
    */
    server.at("/pn/list").get(list_pn);


    /*
    购买PN
    req
    uri: /pn/rent
    method: POST
    body: {
        device: DeviceId,   ood的device id
        pn: DeviceId,       pn的device id
        bandwidth: Number   pn的带宽
    }

    resp:
    body: {
        err: 0 成功/ 4 pn不存在/ 5 已经有了/ 10 满了
    }
    */
    server.at("/pn/rent").post(rent_pn);


    /*
    当前使用的PN
    req
    uri: /pn/query
    method: POST
    body: {
        device: DeviceId, ood的device id
    }

    resp
    body: [{
        pn: DeviceId, 
        bandwith: 10M/20M, 带宽
    }] 没有的话是空数组
    */
    server.at("/pn/query").post(query_pn);


    /*
    取消PN
    req
    uri: /pn/cancel
    method: POST
    body: {
        device: DeviceId,   ood的device id
        pn: DeviceId,       pn的device id
        bandwidth: Number   pn的带宽
    }

    resp:
    body: {
        err: 0 成功/ 4 pn不存在
    }
    */
    server.at("/pn/cancel").post(cancel_pn);


    /*
    购买PN
    req
    uri: /pn/cancel
    method: POST
    body: {
        device: DeviceId,   ood的device id
        pn: DeviceId,       pn的device id
        bandwidth: Number   pn的带宽
    }

    resp:
    body: {
        err: 0 成功/ 4 pn不存在
    }
    */
    server.at("/pn/white_list").post(add_pn_white_list);

    let _ = server.listen(format!("127.0.0.1:{}", port).as_str()).await?;
    Ok(())
}

fn device_id_serialize<S>(id: &DeviceId, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(id.to_string().as_str())
}

fn device_id_deserialize<'de, D>(d: D) -> Result<DeviceId, D::Error> 
where 
    D: Deserializer<'de>,  
{
    struct DeviceIdVisitor {}
    impl<'de> Visitor<'de> for DeviceIdVisitor {
        type Value = DeviceId;
    
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("device id")
        }
    
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DeviceId::from_str(value).map_err(|_| {
                de::Error::invalid_value(de::Unexpected::Str(value), &self)
            })
        }
    }

    d.deserialize_str(DeviceIdVisitor {})
}

async fn list_pn(req: Request<Server>) -> tide::Result {
    #[derive(Serialize)]
    struct PnInfo {
        #[serde(serialize_with = "device_id_serialize")]
        id: DeviceId, 
        bandwidth: u32, 
        used: usize, 
        limit: usize
    }

    let resp: Vec<PnInfo> = req.state().storage().used()
        .map(|u| u.into_iter().map(
            |(bandwidth, used, limit)| PnInfo {
                id: req.state().pn().clone(), 
                bandwidth, 
                used, 
                limit
            }).collect())?;

    Ok(Response::from(Body::from_json(&resp)?))
}

async fn rent_pn(mut req: Request<Server>) -> tide::Result {
    #[derive(Deserialize)]
    struct RentReq {
        #[serde(deserialize_with = "device_id_deserialize")]
        device: DeviceId, 
        #[serde(deserialize_with = "device_id_deserialize")]
        pn: DeviceId, 
        bandwidth: u32
    }

    #[derive(Serialize)]
    struct RentResp {
        err: u16
    }
    
    let rent_req: RentReq = req.body_json().await?;
    let err_code = {
        if rent_req.pn.eq(req.state().pn()) {
            match req.state().storage().rent(rent_req.device, rent_req.bandwidth) {
                Ok(_) => Ok(BuckyErrorCode::Ok), 
                Err(err) => {
                    match err.code() {
                        BuckyErrorCode::NotFound => Ok(err.code()), 
                        BuckyErrorCode::AlreadyExists => Ok(err.code()), 
                        BuckyErrorCode::OutOfLimit => Ok(err.code()), 
                        _ => Err(err)
                    }
                }
            }   
        } else {
            Ok(BuckyErrorCode::NotFound)
        }
    }?;
    
    
    Ok(Response::from(Body::from_json(&RentResp {
        err: err_code.into()
    })?))
}


async fn query_pn(mut req: Request<Server>) -> tide::Result {
    #[derive(Deserialize)]
    struct QueryReq {
        #[serde(deserialize_with = "device_id_deserialize")]
        device: DeviceId
    }

    #[derive(Serialize)]
    struct RentInfo {
        #[serde(serialize_with = "device_id_serialize")]
        id: DeviceId, 
        bandwidth: u32, 
    }

    let query_req: QueryReq = req.body_json().await?;
    
    let resp: Vec<RentInfo> = req.state().storage().contract_of(&query_req.device).
        map(|v| v.into_iter().map(|bandwidth| RentInfo {id: req.state().pn().clone(), bandwidth}).collect())?;
    
    
    Ok(Response::from(Body::from_json(&resp)?))
}



async fn cancel_pn(mut req: Request<Server>) -> tide::Result {
    #[derive(Deserialize)]
    struct CancelReq {
        #[serde(deserialize_with = "device_id_deserialize")]
        device: DeviceId, 
        #[serde(deserialize_with = "device_id_deserialize")]
        pn: DeviceId, 
        bandwidth: u32
    }

    #[derive(Serialize)]
    struct CancelResp {
        err: u16
    }
    
    let cancel_req: CancelReq = req.body_json().await?;
    let err_code = {
        if cancel_req.pn.eq(req.state().pn()) {
            match req.state().storage().cancel(cancel_req.device, cancel_req.bandwidth) {
                Ok(_) => Ok(BuckyErrorCode::Ok), 
                Err(err) => {
                    match err.code() {
                        BuckyErrorCode::NotFound => Ok(err.code()), 
                        _ => Err(err)
                    }
                }
            }   
        } else {
            Ok(BuckyErrorCode::NotFound)
        }
    }?;
    
    
    Ok(Response::from(Body::from_json(&CancelResp {
        err: err_code.into()
    })?))
}



async fn add_pn_white_list(mut req: Request<Server>) -> tide::Result {
    #[derive(Deserialize)]
    struct RentReq {
        #[serde(deserialize_with = "device_id_deserialize")]
        device: DeviceId, 
        #[serde(deserialize_with = "device_id_deserialize")]
        pn: DeviceId, 
        bandwidth: u32
    }

    #[derive(Serialize)]
    struct RentResp {
        err: u16
    }
    
    let rent_req: RentReq = req.body_json().await?;
    let err_code = {
        if rent_req.pn.eq(req.state().pn()) {
            match req.state().storage().add_white_list(rent_req.device) {
                Ok(_) => Ok(BuckyErrorCode::Ok), 
                Err(err) => {
                    match err.code() {
                        BuckyErrorCode::NotFound => Ok(err.code()), 
                        BuckyErrorCode::AlreadyExists => Ok(err.code()), 
                        BuckyErrorCode::OutOfLimit => Ok(err.code()), 
                        _ => Err(err)
                    }
                }
            }   
        } else {
            Ok(BuckyErrorCode::NotFound)
        }
    }?;
    
    
    Ok(Response::from(Body::from_json(&RentResp {
        err: err_code.into()
    })?))
}
