use super::handler::*;
use crate::root_state::*;
use cyfs_lib::*;

use async_trait::async_trait;
use tide::Response;


enum GlobalStateRequestType {
    GetCurrentRoot,
    CreateOpEnv,
}

pub(crate) struct GlobalStateRequestHandlerEndpoint {
    protocol: NONProtocol,
    req_type: GlobalStateRequestType,
    handler: GlobalStateRequestHandler,
}

impl GlobalStateRequestHandlerEndpoint {
    fn new(
        protocol: NONProtocol,
        req_type: GlobalStateRequestType,
        handler: GlobalStateRequestHandler,
    ) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: ::tide::Request<State>) -> Response {
        let req = RootStateInputHttpRequest::new(&self.protocol, req);

        match self.req_type {
            GlobalStateRequestType::GetCurrentRoot => {
                self.handler.process_get_current_root_request(req).await
            }
            GlobalStateRequestType::CreateOpEnv => {
                self.handler.process_create_op_env_request(req).await
            }
        }
    }

    pub fn register_server(
        protocol: &NONProtocol,
        root_seg: &str,
        handler: &GlobalStateRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        let path = format!("/{}/root", root_seg);

        // get_current_root
        server.at(&path).post(GlobalStateRequestHandlerEndpoint::new(
            protocol.to_owned(),
            GlobalStateRequestType::GetCurrentRoot,
            handler.clone(),
        ));

        // create_op_env
        let path = format!("/{}/op-env", root_seg);
        server.at(&path).post(GlobalStateRequestHandlerEndpoint::new(
            protocol.to_owned(),
            GlobalStateRequestType::CreateOpEnv,
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for GlobalStateRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}

enum OpEnvRequestType {
    // create_new/load/load_by_path
    Load,
    LoadByPath,
    CreateNew,

    // transaction
    Commit,
    Abort,

    // lock
    Lock,

    // get_current_root
    GetCurrentRoot,

    // map
    GetByKey,
    InsertWithKey,
    SetWithKey,
    RemoveWithKey,

    // set
    Contains,
    Insert,
    Remove,

    // iterator
    Next,
    Reset,
    List,

    // metadata
    Metadata,
}

/*
POST insert_with_key insert
PUT set_with_key
DELETE remove_with_key remove
GET get_by_key contains
*/

pub(crate) struct OpEnvRequestHandlerEndpoint {
    protocol: NONProtocol,
    req_type: OpEnvRequestType,
    handler: OpEnvRequestHandler,
}

impl OpEnvRequestHandlerEndpoint {
    fn new(
        protocol: NONProtocol,
        req_type: OpEnvRequestType,
        handler: OpEnvRequestHandler,
    ) -> Self {
        Self {
            protocol,
            req_type,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: ::tide::Request<State>) -> Response {
        let req = OpEnvInputHttpRequest::new(&self.protocol, req);

        match self.req_type {
            OpEnvRequestType::Load => self.handler.process_load_request(req).await,
            OpEnvRequestType::LoadByPath => self.handler.process_load_by_path_request(req).await,
            OpEnvRequestType::CreateNew => self.handler.process_create_new_request(req).await,

            OpEnvRequestType::Lock => self.handler.process_lock_request(req).await,

            OpEnvRequestType::GetCurrentRoot => self.handler.process_get_current_root_request(req).await,

            OpEnvRequestType::Commit => self.handler.process_commit_request(req).await,
            OpEnvRequestType::Abort => self.handler.process_abort_request(req).await,

            OpEnvRequestType::GetByKey => self.handler.process_get_by_key_request(req).await,
            OpEnvRequestType::InsertWithKey => {
                self.handler.process_insert_with_key_request(req).await
            }
            OpEnvRequestType::SetWithKey => self.handler.process_set_with_key_request(req).await,
            OpEnvRequestType::RemoveWithKey => {
                self.handler.process_remove_with_key_request(req).await
            }

            OpEnvRequestType::Contains => self.handler.process_contains_request(req).await,
            OpEnvRequestType::Insert => self.handler.process_insert_request(req).await,
            OpEnvRequestType::Remove => self.handler.process_remove_request(req).await,

            OpEnvRequestType::Next => self.handler.process_next_request(req).await,
            OpEnvRequestType::Reset => self.handler.process_reset_request(req).await,
            OpEnvRequestType::List => self.handler.process_list_request(req).await,

            OpEnvRequestType::Metadata => self.handler.process_metadata_request(req).await,
        }
    }

    pub fn register_server(
        protocol: &NONProtocol,
        root_seg: &str,
        handler: &OpEnvRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
        // load
        let path = format!("/{}/op-env/init/target", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Load,
            handler.clone(),
        ));

        // load_by_path
        let path = format!("/{}/op-env/init/path", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::LoadByPath,
            handler.clone(),
        ));

        // create_new
        let path = format!("/{}/op-env/init/new", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::CreateNew,
            handler.clone(),
        ));

        // lock
        let path = format!("/{}/op-env/lock", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Lock,
            handler.clone(),
        ));

        // get_current_root
        let path = format!("/{}/op-env/root", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::GetCurrentRoot,
            handler.clone(),
        ));

        // commit
        let path = format!("/{}/op-env/transaction", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Commit,
            handler.clone(),
        ));
        // abort
        server.at(&path).delete(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Abort,
            handler.clone(),
        ));

        // get_by_key
        let path = format!("/{}/op-env/map", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::GetByKey,
            handler.clone(),
        ));
        // insert_with_key
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::InsertWithKey,
            handler.clone(),
        ));
        // set_with_key
        server.at(&path).put(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::SetWithKey,
            handler.clone(),
        ));
        // remove_with_key
        server.at(&path).delete(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::RemoveWithKey,
            handler.clone(),
        ));

        // contains
        let path = format!("/{}/op-env/set", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Contains,
            handler.clone(),
        ));
        // insert
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Insert,
            handler.clone(),
        ));
        // remove
        server.at(&path).delete(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Remove,
            handler.clone(),
        ));

        // next
        let path = format!("/{}/op-env/iterator", root_seg);
        server.at(&path).post(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Next,
            handler.clone(),
        ));

        server.at(&path).delete(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Reset,
            handler.clone(),
        ));

        // list
        let path = format!("/{}/op-env/list", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::List,
            handler.clone(),
        ));

        // metadata
        let path = format!("/{}/op-env/metadata", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            OpEnvRequestType::Metadata,
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for OpEnvRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}


////// access

pub(crate) struct GlobalStateAccessRequestHandlerEndpoint {
    protocol: NONProtocol,
    handler: GlobalStateAccessRequestHandler,
}

impl GlobalStateAccessRequestHandlerEndpoint {
    fn new(
        protocol: NONProtocol,
        handler: GlobalStateAccessRequestHandler,
    ) -> Self {
        Self {
            protocol,
            handler,
        }
    }

    async fn process_request<State: Send>(&self, req: ::tide::Request<State>) -> Response {
        let req = RootStateInputHttpRequest::new(&self.protocol, req);


        self.handler.process_access_request(req).await
    }

    pub fn register_server(
        protocol: &NONProtocol,
        root_seg: &str,
        handler: &GlobalStateAccessRequestHandler,
        server: &mut ::tide::Server<()>,
    ) {
       
        // get_object_by_path & list
        let path = format!("/{}/*inner_path", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            handler.clone(),
        ));

        let path = format!("/{}/", root_seg);
        server.at(&path).get(Self::new(
            protocol.to_owned(),
            handler.clone(),
        ));
    }
}

#[async_trait]
impl<State: Send> tide::Endpoint<State> for GlobalStateAccessRequestHandlerEndpoint
where
    State: Clone + Send + Sync + 'static,
{
    async fn call(&self, req: ::tide::Request<State>) -> tide::Result {
        let resp = self.process_request(req).await;
        Ok(resp)
    }
}