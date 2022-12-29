use cyfs_base::*;
use async_std::net::TcpStream;
use http_types::{Response};

use crate::chunk_request;
use crate::chunk_client_context::*;
use log::*;

pub struct ChunkClient {
    // ignore
}

impl ChunkClient {
    pub async fn request_non_block<T>(ctx: impl ChunkClientContext, func:&str, t: &T) -> BuckyResult<Response> 
        where 
        T: RawEncode
    {
        let req = chunk_request::create_post_request(ctx.get_end_point().to_string(), func, t)?;
        let ep = ctx.get_end_point().to_owned();
        match ctx.get_bdt_stream() {
            Some(stream)=>{
                info!("request to:{} by bdt", ep);
                let resp = chunk_request::process_req(req, stream).await?;
                info!("response from:{} by bdt", ep);
                Ok(resp)
            },
            None=>{
                let conn_str = format!("{}:{}", req.url().host_str().unwrap(), req.url().port_or_known_default().unwrap_or(80));
                let stream = TcpStream::connect(&conn_str).await?;
                info!("request to:{}", conn_str);
                let resp = chunk_request::process_req(req, stream).await?;
                info!("response from:{}", conn_str);
                Ok(resp)
            }
        }
    }

    pub async fn request<T>(ctx: impl ChunkClientContext, func:&str, t: &T) -> BuckyResult<Vec<u8>> 
        where 
        T: RawEncode
    {
        let mut resp = Self::request_non_block(ctx, func, t).await?;
        // 在某些情况下，会出现udp传输一段时间后，不能再收到包的情况。在这里加一个10分钟的超时，如果超时了，返回给上层TimeOut错误，上层可以根据错误来考虑重试
        let bytes = async_std::future::timeout(std::time::Duration::from_secs(60*10), async {
            resp.body_bytes().await.map_err(|e|{
                error!("receive bytes error, msg:{}", e.to_string());
                BuckyError::from(e)
            })
        }).await.map_err(|e| {
            error!("recv chunk timeout.");
            BuckyError::from(e)
        })??;

        Ok(bytes)
    }

    pub async fn set(ctx: ChunkSourceContext, chunk_req: & cyfs_chunk::ChunkSetReq)->BuckyResult<cyfs_chunk::ChunkSetResp>{
        let bytes = ChunkClient::request(ctx, cyfs_chunk::method::SET_CHUNK, chunk_req).await?;
        let resp = cyfs_chunk::ChunkSetResp::clone_from_slice(&bytes)?;
        Ok(resp)
    }

    pub async fn get_resp_from_source(ctx: ChunkSourceContext, chunk_req: & cyfs_chunk::ChunkGetReq)->BuckyResult<Response>{
        let resp = ChunkClient::request_non_block(ctx, cyfs_chunk::method::GET_CHUNK, chunk_req).await?;
        Ok(resp)
    }

    pub async fn get_from_source(ctx: ChunkSourceContext, chunk_req: & cyfs_chunk::ChunkGetReq)->BuckyResult<cyfs_chunk::ChunkGetResp>{
        let bytes = ChunkClient::request(ctx, cyfs_chunk::method::GET_CHUNK, chunk_req).await?;
        let resp = cyfs_chunk::ChunkGetResp::clone_from_slice(&bytes).map_err(|e|{
            error!("decode ChunkGetResp failed");
            e
        })?;
        Ok(resp)
    }

    // pub async fn create_delegate(ctx: ChunkSourceContext, chunk_req: & chunk::ChunkCreateDelegateReq)->BuckyResult<chunk::ChunkCreateDelegateResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::CREATE_CHUNK_DELEGATE, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn query_delegate(ctx: ChunkSourceContext, chunk_req: & chunk::ChunkCreateDelegateQueryReq)->BuckyResult<chunk::ChunkCreateDelegateQueryResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::QUERY_CHUNK_DELEGATE, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn delegate(ctx: ChunkCacheContext, chunk_req: & chunk::ChunkDelegateReq)->BuckyResult<chunk::ChunkDelegateResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::DELEGATE_CHUNK, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn get_from_cache(ctx: ChunkCacheContext, chunk_req: & chunk::ChunkCacheReq)->BuckyResult<chunk::ChunkCacheResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::GET_CHUNK_CACHE, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn redirect(ctx: ChunkCacheContext, chunk_req: & chunk::ChunkRedirectReq)->BuckyResult<chunk::ChunkCacheResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::REDIRECT_CHUNK, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn proof(ctx: ChunkCacheContext, chunk_req: & chunk::ChunkProofReq)->BuckyResult<chunk::ChunkProofResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::PROOF_CHUNK, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }

    // pub async fn redeem(ctx: ChunkSourceContext, chunk_req: & chunk::ChunkRedeemReq)->BuckyResult<chunk::ChunkRedeemResp>{
    //     let bytes = ChunkClient::request(ctx, chunk::method::REDEEM_CHUNK_PROOF, chunk_req).await?;
    //     let resp = chunk::Deserializer::from_vec(&bytes)?;
    //     Ok(resp)
    // }
}