use async_trait::async_trait;
use tide::Response;
use cyfs_lib::RequestProtocol;
use crate::non::NONInputHttpRequest;

use crate::trans_api::service::trans_handler::*;
use crate::ZoneManagerRef;

enum TransRequestType {
    CreateTask,
    ControlTask,
    GetTaskState,
    PublishFile,
    GetContext,
    PutContext,
    QueryTasks,

    ControlTaskGroup,
    GetTaskGroupState,
}

pub(crate) struct TransRequestHandlerEndpoint {
    zone_manager: ZoneManagerRef,
    protocol: RequestProtocol,
    req_type: TransRequestType,
    handler: TransRequestHandler,
}

impl TransRequestHandlerEndpoint {
    fn new(zone_manager: ZoneManagerRef,
           protocol: RequestProtocol,
           req_type: TransRequestType,
           handler: TransRequestHandler) -> Self {
        Self { zone_manager, protocol, req_type, handler }
    }

    async fn process_request<State>(&self, req: tide::Request<State>) -> Response {
        let req = match NONInputHttpRequest::new(&self.zone_manager, &self.protocol, req).await
        {
            Ok(v) => v,
            Err(resp) => return resp,
        };
        match self.req_type {
            TransRequestType::CreateTask => self.handler.process_create_task(req).await,
            TransRequestType::ControlTask => self.handler.process_control_task(req).await,
            TransRequestType::GetTaskState => self.handler.process_get_task_state(req).await,
            TransRequestType::PublishFile => self.handler.process_publish_file(req).await,
            TransRequestType::GetContext => self.handler.process_get_context(req).await,
            TransRequestType::PutContext => self.handler.process_put_context(req).await,
            TransRequestType::QueryTasks => self.handler.process_query_tasks_context(req).await,

            TransRequestType::ControlTaskGroup => self.handler.process_control_task_group(req).await,
            TransRequestType::GetTaskGroupState => self.handler.process_get_task_group_state(req).await,
        }
    }

    pub fn register_server(zone_manager: &ZoneManagerRef,
                           protocol: &RequestProtocol,
                           handler: &TransRequestHandler,
                           server: &mut tide::Server<()>) {
        server
            .at("/trans/get_context")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::GetContext, handler.clone()));

        server
            .at("trans/put_context")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::PutContext, handler.clone()));

        server
            .at("/trans/task")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::CreateTask, handler.clone()));

        server
            .at("/trans/task")
            .put(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::ControlTask, handler.clone()));

        server
            .at("/trans/task/state")
            .get(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::GetTaskState, handler.clone()));
        server
            .at("/trans/task/state")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::GetTaskState, handler.clone()));

        server
            .at("/trans/file")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::PublishFile, handler.clone()));

        server
            .at("/trans/query")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::QueryTasks, handler.clone()));


        // task group
        server
            .at("/trans/task_group/state")
            .post(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::GetTaskGroupState, handler.clone()));

        server
            .at("/trans/task_group")
            .put(Self::new(zone_manager.clone(), protocol.to_owned(), TransRequestType::ControlTaskGroup, handler.clone()));

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
