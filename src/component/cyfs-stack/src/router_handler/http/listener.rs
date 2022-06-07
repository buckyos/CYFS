use super::handler::*;
use cyfs_lib::RequestorHelper;

use async_trait::async_trait;
use tide::{Response, StatusCode};

enum RouterHandlerRequestType {
    AddHandler,
    RemoveHandler,
}

pub(crate) struct RouterHandlerRequestHandlerEndpoint {
    req_type: RouterHandlerRequestType,
    handler: RouterHandlerHttpHandler,
}

impl RouterHandlerRequestHandlerEndpoint {
    fn new(req_type: RouterHandlerRequestType, handler: RouterHandlerHttpHandler) -> Self {
        Self { req_type, handler }
    }

    async fn process_request<State>(&self, mut req: ::tide::Request<State>) -> Response {
        match req.body_string().await {
            Ok(body) => match self.req_type {
                RouterHandlerRequestType::AddHandler => self.handler.process_add_handler(req, body).await,
                RouterHandlerRequestType::RemoveHandler => {
                    self.handler.process_remove_handler(req, body).await
                }
            },

            Err(e) => {
                error!("read router handler body error! err={}", e);

                RequestorHelper::new_response(StatusCode::BadRequest).into()
            }
        }
    }

    pub fn register_server(handler: &RouterHandlerHttpHandler, server: &mut ::tide::Server<()>) {
        // add_handler
        server.at("/handler/non/:handler_chain/:handler_category/:handler_id").post(
            RouterHandlerRequestHandlerEndpoint::new(RouterHandlerRequestType::AddHandler, handler.clone()),
        );

        // remove_handler
        server.at("/handler/non/:handler_chain/:handler_category/:handler_id").delete(
            RouterHandlerRequestHandlerEndpoint::new(
                RouterHandlerRequestType::RemoveHandler,
                handler.clone(),
            ),
        );
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for RouterHandlerRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
