use super::handler::*;
use cyfs_lib::*;

use async_trait::async_trait;
use tide::Response;

enum NDNRequestType {
    PutData,
    Get,
    DownloadData,
    DeleteData,
}

pub(crate) struct NDNRequestHandlerEndpoint {
    protocol: NONProtocol,
    req_type: NDNRequestType,
    handler: NDNRequestHandler,
}

impl NDNRequestHandlerEndpoint {
    fn new(protocol: NONProtocol, req_type: NDNRequestType, handler: NDNRequestHandler) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State>(&self, req: ::tide::Request<State>) -> Response {
        let req = NDNInputHttpRequest::new(&self.protocol, req);

        match self.req_type {
            NDNRequestType::Get => self.handler.process_get_request(req).await,
            NDNRequestType::DownloadData => self.handler.process_download_data_request(req).await,
            NDNRequestType::PutData => self.handler.process_put_data_request(req).await,
            NDNRequestType::DeleteData => self.handler.process_delete_data_request(req).await,
        }
    }

    pub fn register_server(
        protocol: &NONProtocol,
        handler: &NDNRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        // get_data/query_file
        server.at("/ndn/*must").post(NDNRequestHandlerEndpoint::new(
            protocol.to_owned(),
            NDNRequestType::Get,
            handler.clone(),
        ));

        server.at("/ndn/").post(NDNRequestHandlerEndpoint::new(
            protocol.to_owned(),
            NDNRequestType::Get,
            handler.clone(),
        ));

        server.at("/ndn/*must").get(NDNRequestHandlerEndpoint::new(
            protocol.to_owned(),
            NDNRequestType::DownloadData,
            handler.clone(),
        ));

        // put_data
        server.at("/ndn/*must").put(NDNRequestHandlerEndpoint::new(
            protocol.to_owned(),
            NDNRequestType::PutData,
            handler.clone(),
        ));

        // delete_data
        server
            .at("/ndn/*must")
            .delete(NDNRequestHandlerEndpoint::new(
                protocol.to_owned(),
                NDNRequestType::DeleteData,
                handler.clone(),
            ));
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for NDNRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
