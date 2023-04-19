use http_types::{Body, StatusCode};
use tide::{Request, Response};

use cyfs_base::*;
use cyfs_lib::{NDNAPILevel, NDNGetDataRequest, NDNPutDataRequest, SharedCyfsStack};
use log::*;

pub struct ChunkInterface {
    stack: SharedCyfsStack,
}

impl ChunkInterface {
    pub fn new(stack: SharedCyfsStack)->ChunkInterface{
        ChunkInterface {
            stack,
        }
    }

    pub async fn run(&self) -> Result<(), std::io::Error>  {
        let mut app = tide::with_state((self.stack.clone(), PrivateKey::generate_secp256k1().unwrap()));

        app.at(cyfs_chunk::method_path::GET_CHUNK).post(move |mut req: Request<(SharedCyfsStack, PrivateKey)>| {
            async move {
                let body = req.body_bytes().await?;
                let chunk_get_req = cyfs_chunk::ChunkGetReq::clone_from_slice(&body)?;
                let resp = req.state().0.ndn_service().get_data(NDNGetDataRequest::new_ndc(chunk_get_req.chunk_id().object_id(), None)).await?;
                let mut http_resp = Response::new(StatusCode::Ok);
                http_resp.set_body(Body::from_reader(async_std::io::BufReader::new(resp.data), Some(resp.length as usize)));
                Ok(http_resp)
            }
        });

        app.at(cyfs_chunk::method_path::SET_CHUNK).post(move |mut req: Request<(SharedCyfsStack, PrivateKey)>| {
            async move {
                let body = req.body_bytes().await?;
                let chunk_set_req = cyfs_chunk::ChunkSetReq::clone_from_slice(&body)?;
                let resp = req.state().0.ndn_service()
                    .put_data(NDNPutDataRequest::new_with_buffer(NDNAPILevel::NDC, chunk_set_req.chunk_id().object_id(),
                                                         chunk_set_req.data().to_owned())).await?;
                info!("chunk manager put chunk {} result {}", chunk_set_req.chunk_id(), resp.result.to_string());
                // here set a fake sign data because verify always return true
                let chunk_set_resp = cyfs_chunk::ChunkSetResp::sign(
                    &req.state().1,
                    &DeviceId::default(),
                    chunk_set_req.chunk_id(),
                    cyfs_chunk::ChunkSetStatus::Ok
                )?;

                let resp_str = chunk_set_resp.to_vec()?;
                let mut http_resp = Response::new(StatusCode::Ok);
                http_resp.set_body(resp_str);
                Ok(http_resp)
            }
        });

        app.at(cyfs_chunk::method_path::CREATE_CHUNK_DELEGATE).post(move |mut _req: Request<(SharedCyfsStack, PrivateKey)>| {
            async move {
                Ok(Response::new(StatusCode::Ok))
            }
        });

        app.at(cyfs_chunk::method_path::REDEEM_CHUNK_PROOF).post(move |mut _req: Request<(SharedCyfsStack, PrivateKey)>| {
            async move {
                Ok(Response::new(StatusCode::Ok))
            }
        });

        app.at(cyfs_chunk::method_path::QUERY_CHUNK_DELEGATE).post(move |mut _req: Request<(SharedCyfsStack, PrivateKey)>| {
            async move {
                Ok(Response::new(StatusCode::Ok))
            }
        });

        let addr = format!("127.0.0.1:{}", ::cyfs_base::CHUNK_MANAGER_PORT);
        app.listen(addr).await?;

        Ok(())
    }
}
