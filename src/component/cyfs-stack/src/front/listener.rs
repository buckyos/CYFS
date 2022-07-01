use super::protocol::FrontProtocolHandlerRef;
/// use super::handler::*;
use cyfs_lib::*;

use async_trait::async_trait;


pub(crate) type FrontInputHttpRequest<State> = crate::non::NONInputHttpRequest<State>;

#[derive(Clone, Copy)]
pub enum FrontRequestType {
    O,
    R,
    L,
    A,

    // treat as o protocol
    Any,
}

pub(crate) struct FrontRequestHandlerEndpoint {
    protocol: NONProtocol,
    req_type: FrontRequestType,
    handler: FrontProtocolHandlerRef,
}

impl FrontRequestHandlerEndpoint {
    fn new(
        protocol: NONProtocol,
        req_type: FrontRequestType,
        handler: FrontProtocolHandlerRef,
    ) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: tide::Request<State>) -> tide::Response {
        let req = FrontInputHttpRequest::new(&self.protocol, req);

        self.handler.process_request(self.req_type, req).await
    }

    pub fn register_server(
        protocol: &NONProtocol,
        handler: &FrontProtocolHandlerRef,
        server: &mut ::tide::Server<()>,
    ) {
        // o protocol
        server.at("/o/*must").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::O,
            handler.clone(),
        ));

        // r protocol
        server.at("/r/*must").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::R,
            handler.clone(),
        ));
        server.at("/l/*must").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::L,
            handler.clone(),
        ));

        // a
        server.at("/a/*must").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::A,
            handler.clone(),
        ));

        // any
        server.at("/:name/*must").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::Any,
            handler.clone(),
        ));

        server.at("/:name").get(FrontRequestHandlerEndpoint::new(
            protocol.to_owned(),
            FrontRequestType::Any,
            handler.clone(),
        ));

    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for FrontRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
