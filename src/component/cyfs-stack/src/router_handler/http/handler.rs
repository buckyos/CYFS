use super::super::RouterHandlersManager;
use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

pub(crate) struct RouterHandlerHttpHandler {
    protocol: RequestProtocol,

    processor: RouterHandlerHttpProcessor,
}

impl Clone for RouterHandlerHttpHandler {
    fn clone(&self) -> Self {
        Self {
            protocol: self.protocol.clone(),
            processor: self.processor.clone(),
        }
    }
}

impl RouterHandlerHttpHandler {
    pub fn new(protocol: RequestProtocol, manager: RouterHandlersManager) -> Self {
        let processor = RouterHandlerHttpProcessor::new(manager);
        Self {
            protocol,
            processor,
        }
    }

    fn extract_id_from_path<State>(
        req: &tide::Request<State>,
    ) -> BuckyResult<(RouterHandlerChain, RouterHandlerCategory, String)> {
        // 提取路径上的handler_chain+handler_category+handler_id
        let handler_chain: RouterHandlerChain = req
            .param("handler_chain")
            .map_err(|e| {
                let msg = format!("invalid handler_chain: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .parse()?;

        let handler_category: RouterHandlerCategory = req
            .param("handler_category")
            .map_err(|e| {
                let msg = format!("invalid handler_category: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .parse()?;

        let handler_id: String = req
            .param("handler_id")
            .map_err(|e| {
                let msg = format!("invalid handler_id: {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?
            .to_owned();

        Ok((handler_chain, handler_category, handler_id))
    }

    pub async fn process_add_handler<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> tide::Response {
        let ret = self.on_add_handler_request(req, body).await;
        match ret {
            Ok(_) => RequestorHelper::new_ok_response(),
            Err(e) => {
                error!("router add handler error: {}", e);
                RequestorHelper::trans_error(e)
            }
        }
    }

    pub async fn process_remove_handler<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> tide::Response {
        let resp = self.on_remove_handler_request(req, body).await;
        match resp {
            Ok(ret) => {
                if ret {
                    RequestorHelper::new_ok_response()
                } else {
                    let e = BuckyError::from(BuckyErrorCode::NotFound);
                    RequestorHelper::trans_error(e)
                }
            }
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn on_add_handler_request<State>(
        &self,
        req: tide::Request<State>,
        body: String,
    ) -> BuckyResult<()> {
        let (chain, category, id) = Self::extract_id_from_path(&req)?;

        let param = RouterAddHandlerParam::decode_string(&body)?;

        // 提取来源device
        let source = RequestorHelper::decode_optional_header(&req, cyfs_base::CYFS_REMOTE_DEVICE)?;

        // extrac source dec_id
        let dec_id = RequestorHelper::decode_optional_header(&req, cyfs_base::CYFS_DEC_ID)?;

        info!(
            "recv add handler request: chain={}, category={}, id={}, dec={:?}, source={:?}, body={}",
            chain, category, id, dec_id, source, body
        );

        let add_req = RouterAddHandlerRequest {
            protocol: self.protocol.clone(),
            chain,
            category,
            id,
            dec_id,
            param,
            source,
        };

        let mut source = RequestSourceInfo::new_local_dec(add_req.dec_id.clone());
        source.protocol = self.protocol;

        self.processor.on_add_handler_request(source, add_req).await
    }

    async fn on_remove_handler_request<State>(
        &self,
        req: tide::Request<State>,
        _body: String,
    ) -> BuckyResult<bool> {
        let (chain, category, id) = Self::extract_id_from_path(&req)?;

        // 提取来源device
        let source = RequestorHelper::decode_optional_header(&req, cyfs_base::CYFS_REMOTE_DEVICE)?;

        // extrac source dec_id
        let dec_id = RequestorHelper::decode_optional_header(&req, cyfs_base::CYFS_DEC_ID)?;

        info!(
            "recv remove handler request: chain={}, category={}, id={}, dec={:?}, source={:?}",
            chain, category, id, dec_id, source,
        );

        let remove_req = RouterRemoveHandlerRequest {
            protocol: self.protocol.clone(),
            chain,
            category,
            id,
            dec_id,
            source,
        };

        self.processor.on_remove_handler_request(remove_req).await
    }
}
