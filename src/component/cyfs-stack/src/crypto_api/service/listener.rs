use super::handler::*;
use crate::non::NONInputHttpRequest;
use cyfs_lib::*;
 
use async_trait::async_trait;
use tide::{Response, StatusCode};

enum CryptoRequestType {
    SignObject,
    VerifyObject,
}

pub(crate) struct CryptoRequestHandlerEndpoint {
    protocol: NONProtocol, 
    req_type: CryptoRequestType,
    handler: CryptoRequestHandler,
}

impl CryptoRequestHandlerEndpoint {
    fn new(protocol: NONProtocol, req_type: CryptoRequestType, handler: CryptoRequestHandler) -> Self {
        Self { protocol, req_type, handler }
    }

    async fn process_request<State>(&self, req: ::tide::Request<State>) -> Response {
        let mut req = NONInputHttpRequest::new(&self.protocol, req);

        match self.req_type {
            CryptoRequestType::VerifyObject => match req.request.body_bytes().await {
                Ok(body) => self.handler.process_verify_object(req, body).await,

                Err(e) => {
                    error!("read crypto verify object request body error! err={}", e);

                    RequestorHelper::new_response(StatusCode::BadRequest).into()
                }
            },
            CryptoRequestType::SignObject => match req.request.body_bytes().await {
                Ok(body) => self.handler.process_sign_object(req, body).await,

                Err(e) => {
                    error!("read crypto sign object request body error! err={}", e);

                    RequestorHelper::new_response(StatusCode::BadRequest).into()
                }
            },
        }
    }

    pub fn register_server(protocol: &NONProtocol, handler: &CryptoRequestHandler, server: &mut ::tide::Server<()>) {
        // verify_object
        let mut route = server.at("/crypto/verify/*must");
        route.get(CryptoRequestHandlerEndpoint::new(
            protocol.to_owned(),
            CryptoRequestType::VerifyObject,
            handler.clone(),
        ));
        route.post(CryptoRequestHandlerEndpoint::new(
            protocol.to_owned(),
            CryptoRequestType::VerifyObject,
            handler.clone(),
        ));

        // sign_object
        let mut route = server.at("/crypto/sign/*must");
        route.get(CryptoRequestHandlerEndpoint::new(
            protocol.to_owned(),
            CryptoRequestType::SignObject,
            handler.clone(),
        ));
        route.post(CryptoRequestHandlerEndpoint::new(
            protocol.to_owned(),
            CryptoRequestType::SignObject,
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for CryptoRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
