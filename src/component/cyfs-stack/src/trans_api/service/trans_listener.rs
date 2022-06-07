use async_trait::async_trait;
use tide::Response;

use crate::trans_api::service::trans_handler::*;

enum TransRequestType {
    CreateTask,
    ControlTask,
    GetTaskState,
    PublishFile,
    GetContext,
    PutContext,
    QueryTasks,
}

pub(crate) struct TransRequestHandlerEndpoint {
    req_type: TransRequestType,
    handler: TransRequestHandler,
}

impl TransRequestHandlerEndpoint {
    fn new(req_type: TransRequestType, handler: TransRequestHandler) -> Self {
        Self { req_type, handler }
    }

    async fn process_request<State>(&self, req: tide::Request<State>) -> Response {
        match self.req_type {
            TransRequestType::CreateTask => self.handler.process_create_task(req).await,
            TransRequestType::ControlTask => self.handler.process_control_task(req).await,
            TransRequestType::GetTaskState => self.handler.process_get_task_state(req).await,
            TransRequestType::PublishFile => self.handler.process_publish_file(req).await,
            TransRequestType::GetContext => self.handler.process_get_context(req).await,
            TransRequestType::PutContext => self.handler.process_put_context(req).await,
            TransRequestType::QueryTasks => self.handler.process_query_tasks_context(req).await,
        }
    }

    pub fn register_server(handler: &TransRequestHandler, server: &mut tide::Server<()>) {
        server
            .at("/trans/get_context")
            .post(Self::new(TransRequestType::GetContext, handler.clone()));

        server
            .at("trans/put_context")
            .post(Self::new(TransRequestType::PutContext, handler.clone()));

        server
            .at("/trans/task")
            .post(Self::new(TransRequestType::CreateTask, handler.clone()));

        server
            .at("/trans/task")
            .put(Self::new(TransRequestType::ControlTask, handler.clone()));

        server
            .at("/trans/task/state")
            .get(Self::new(TransRequestType::GetTaskState, handler.clone()));
        server
            .at("/trans/task/state")
            .post(Self::new(TransRequestType::GetTaskState, handler.clone()));

        server
            .at("/trans/file")
            .post(Self::new(TransRequestType::PublishFile, handler.clone()));

        server
            .at("/trans/query")
            .post(Self::new(TransRequestType::QueryTasks, handler.clone()));
    }
}

#[async_trait]
impl<State> tide::Endpoint<State> for TransRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}
