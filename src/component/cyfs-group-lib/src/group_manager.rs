use std::{collections::HashMap, sync::Arc};

use async_std::sync::RwLock;
use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId, OwnerObjectDesc,
    RawConvertTo, RawDecode,
};
use cyfs_core::{
    CoreObjectType, DecAppId, GroupConsensusBlock, GroupConsensusBlockObject, GroupProposalObject,
    GroupRPath,
};
use cyfs_lib::{
    CyfsStackRequestorType, DeviceZoneCategory, HttpRequestorRef, NONObjectInfo,
    NONPostObjectInputResponse, RequestGlobalStatePath, RouterHandlerAction, RouterHandlerChain,
    RouterHandlerManagerProcessor, RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult,
    SharedCyfsStack,
};
use cyfs_util::EventListenerAsyncRoutine;

use crate::{
    DelegateFactory, ExecuteResult, GroupCommand, GroupCommandCommited, GroupCommandExecute,
    GroupCommandExecuteResult, GroupCommandNewRPath, GroupCommandObject, GroupCommandType,
    GroupCommandVerify, RPathClient, RPathDelegate, RPathService,
};

type ServiceByRPath = HashMap<String, RPathService>;
type ServiceByDec = HashMap<ObjectId, ServiceByRPath>;
type ServiceByGroup = HashMap<ObjectId, ServiceByDec>;

type ClientByRPath = HashMap<String, RPathClient>;
type ClientByDec = HashMap<ObjectId, ClientByRPath>;
type ClientByGroup = HashMap<ObjectId, ClientByDec>;

struct GroupManagerRaw {
    stack: SharedCyfsStack,
    requestor: HttpRequestorRef,
    delegate_factory: Option<Box<dyn DelegateFactory>>,
    clients: RwLock<ClientByGroup>,
    services: RwLock<ServiceByGroup>,
    local_zone: Option<ObjectId>,
}

#[derive(Clone)]
pub struct GroupManager(Arc<GroupManagerRaw>);

impl GroupManager {
    pub async fn open(
        stack: SharedCyfsStack,
        delegate_factory: Box<dyn DelegateFactory>,
        requestor_type: &CyfsStackRequestorType,
    ) -> BuckyResult<Self> {
        if stack.dec_id().is_none() {
            let msg = "the stack should be opened with dec-id";
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let dec_id = stack.dec_id().unwrap().clone();
        let requestor = stack.select_requestor(requestor_type);
        let local_zone = stack.local_device().desc().owner().clone();
        let router_handler_manager = stack.router_handlers().clone();

        let mgr = Self(Arc::new(GroupManagerRaw {
            stack,
            requestor,
            delegate_factory: Some(delegate_factory),
            clients: RwLock::new(HashMap::new()),
            services: RwLock::new(HashMap::new()),
            local_zone,
        }));

        // TODO: other filters? only local zone
        let filter = format!(
            "obj_type == {} && source.dec_id == {} && source.zone_category == {}",
            CoreObjectType::GroupCommand as u16,
            dec_id,
            DeviceZoneCategory::CurrentZone.to_string(),
        );

        // let filter = "*".to_string();
        let req_path = RequestGlobalStatePath::new(Some(dec_id.clone()), Some("group/inner-cmd"));

        router_handler_manager
            .post_object()
            .add_handler(
                RouterHandlerChain::Handler,
                format!("group-cmd-{}", dec_id).as_str(),
                0,
                Some(filter),
                Some(req_path.format_string()),
                RouterHandlerAction::Pass,
                Some(Box::new(mgr.clone())),
            )
            .await?;

        Ok(mgr)
    }

    pub async fn open_as_client(
        stack: SharedCyfsStack,
        requestor_type: &CyfsStackRequestorType,
    ) -> BuckyResult<Self> {
        let requestor = stack.select_requestor(requestor_type);
        let local_zone = stack.local_device().desc().owner().clone();

        Ok(Self(Arc::new(GroupManagerRaw {
            stack,
            requestor,
            delegate_factory: None,
            clients: RwLock::new(HashMap::new()),
            services: RwLock::new(HashMap::new()),
            local_zone,
        })))
    }

    pub async fn stop(&self) {
        unimplemented!()
    }

    pub fn stack(&self) -> &SharedCyfsStack {
        &self.0.stack
    }

    pub async fn start_rpath_service(
        &self,
        group_id: ObjectId,
        rpath: String,
        delegate: Box<dyn RPathDelegate>,
    ) -> BuckyResult<RPathService> {
        let dec_id = self.0.stack.dec_id().unwrap().clone();

        {
            let services = self.0.services.read().await;
            let found = services
                .get(&group_id)
                .and_then(|by_dec| by_dec.get(&dec_id))
                .and_then(|by_rpath| by_rpath.get(rpath.as_str()));

            if let Some(found) = found {
                return Ok(found.clone());
            }
        }

        {
            let mut services = self.0.services.write().await;
            let service = services
                .entry(group_id.clone())
                .or_insert_with(HashMap::new)
                .entry(dec_id.into())
                .or_insert_with(HashMap::new)
                .entry(rpath.to_string())
                .or_insert_with(|| {
                    RPathService::new(
                        GroupRPath::new(group_id.clone(), dec_id.clone(), rpath.to_string()),
                        self.0.requestor.clone(),
                        delegate,
                        self.0.stack.clone(),
                    )
                });
            Ok(service.clone())
        }
    }

    pub async fn find_rpath_service(
        &self,
        group_id: &ObjectId,
        rpath: &str,
    ) -> BuckyResult<RPathService> {
        let dec_id = self.0.stack.dec_id().unwrap();
        let services = self.0.services.read().await;
        let found = services
            .get(&group_id)
            .and_then(|by_dec| by_dec.get(dec_id))
            .and_then(|by_rpath| by_rpath.get(rpath));

        found.map_or(
            Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                "please start the service first",
            )),
            |service| Ok(service.clone()),
        )
    }

    pub async fn rpath_client(
        &self,
        group_id: ObjectId,
        dec_id: DecAppId,
        rpath: &str,
    ) -> RPathClient {
        {
            let clients = self.0.clients.read().await;
            let found = clients
                .get(&group_id)
                .and_then(|by_dec| by_dec.get(dec_id.object_id()))
                .and_then(|by_rpath| by_rpath.get(rpath));

            if let Some(found) = found {
                return found.clone();
            }
        }

        {
            let client = RPathClient::new(
                GroupRPath::new(group_id, dec_id.object_id().clone(), rpath.to_string()),
                self.0.stack.dec_id().cloned(),
                self.0.stack.non_service().clone(),
            );

            let mut clients = self.0.clients.write().await;
            let client = clients
                .entry(group_id)
                .or_insert_with(HashMap::new)
                .entry(dec_id.into())
                .or_insert_with(HashMap::new)
                .entry(rpath.to_string())
                .or_insert(client);
            client.clone()
        }
    }

    async fn on_command(
        &self,
        cmd: GroupCommand,
    ) -> BuckyResult<Option<GroupCommandExecuteResult>> {
        match cmd.into_cmd() {
            crate::GroupCommandBodyContent::NewRPath(cmd) => {
                self.on_new_rpath(cmd).await.map(|_| None)
            }
            crate::GroupCommandBodyContent::Execute(cmd) => {
                self.on_execute(cmd).await.map(|r| Some(r))
            }
            crate::GroupCommandBodyContent::ExecuteResult(_) => {
                let msg = format!(
                    "should not get the cmd({:?}) in sdk",
                    GroupCommandType::ExecuteResult
                );
                log::warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
            crate::GroupCommandBodyContent::Verify(cmd) => self.on_verify(cmd).await.map(|_| None),
            crate::GroupCommandBodyContent::Commited(cmd) => {
                self.on_commited(cmd).await.map(|_| None)
            }
        }
    }

    async fn on_new_rpath(&self, cmd: GroupCommandNewRPath) -> BuckyResult<()> {
        self.find_or_restart_service(
            &cmd.group_id,
            self.0.stack.dec_id().unwrap(),
            cmd.rpath.as_str(),
            &cmd.with_block,
            true,
        )
        .await
        .map(|_| ())
        .map_err(|err| {
            log::warn!(
                "group on_new_rpath {}-{:?}-{} failed, err: {:?}",
                cmd.group_id,
                self.0.stack.dec_id(),
                cmd.rpath,
                err
            );
            err
        })
    }

    async fn on_execute(&self, cmd: GroupCommandExecute) -> BuckyResult<GroupCommandExecuteResult> {
        let rpath = cmd.proposal.rpath();
        let service = self
            .find_or_restart_service(
                rpath.group_id(),
                rpath.dec_id(),
                rpath.rpath(),
                &None,
                false,
            )
            .await
            .map_err(|err| {
                log::warn!(
                    "group on_execute find service {:?} failed, err: {:?}",
                    cmd.proposal.rpath(),
                    err
                );
                err
            })?;

        let mut result = service
            .on_execute(&cmd.proposal, &cmd.prev_state_id)
            .await
            .map_err(|err| {
                log::warn!(
                    "group on_execute {:?} failed, err: {:?}",
                    cmd.proposal.rpath(),
                    err
                );
                err
            })?;

        Ok(GroupCommandExecuteResult {
            result_state_id: result.result_state_id.take(),
            receipt: result.receipt.take(),
            context: result.context.take(),
        })
    }

    async fn on_verify(&self, mut cmd: GroupCommandVerify) -> BuckyResult<()> {
        let rpath = cmd.proposal.rpath();
        let service = self
            .find_or_restart_service(
                rpath.group_id(),
                rpath.dec_id(),
                rpath.rpath(),
                &None,
                false,
            )
            .await
            .map_err(|err| {
                log::warn!(
                    "group on_verify find service {:?} failed, err: {:?}",
                    cmd.proposal.rpath(),
                    err
                );
                err
            })?;

        let result = ExecuteResult {
            result_state_id: cmd.result_state_id.take(),
            receipt: cmd.receipt.take(),
            context: cmd.context.take(),
        };

        service
            .on_verify(&cmd.proposal, &cmd.prev_state_id, &result)
            .await
            .map_err(|err| {
                log::warn!(
                    "group on_verify {:?} failed, err: {:?}",
                    cmd.proposal.rpath(),
                    err
                );
                err
            })
    }

    async fn on_commited(&self, mut cmd: GroupCommandCommited) -> BuckyResult<()> {
        let rpath = cmd.block.rpath();
        let service = self
            .find_or_restart_service(
                rpath.group_id(),
                rpath.dec_id(),
                rpath.rpath(),
                &None,
                false,
            )
            .await
            .map_err(|err| {
                log::warn!(
                    "group on_commited find service {:?} failed, err: {:?}",
                    cmd.block.rpath(),
                    err
                );
                err
            })?;

        service.on_commited(&cmd.prev_state_id, &cmd.block).await;
        Ok(())
    }

    async fn find_or_restart_service(
        &self,
        group_id: &ObjectId,
        dec_id: &ObjectId,
        rpath: &str,
        with_block: &Option<GroupConsensusBlock>,
        is_new: bool,
    ) -> BuckyResult<RPathService> {
        if dec_id != self.0.stack.dec_id().unwrap() {
            let msg = format!(
                "try find proposal in different dec {:?}, expected: {:?}",
                dec_id,
                self.0.stack.dec_id().unwrap()
            );
            log::warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        {
            let services = self.0.services.read().await;
            let found = services
                .get(group_id)
                .and_then(|by_dec| by_dec.get(dec_id))
                .and_then(|by_rpath| by_rpath.get(rpath));

            if let Some(found) = found {
                return Ok(found.clone());
            }
        }

        match self.0.delegate_factory.as_ref() {
            Some(factory) => {
                let delegate = factory
                    .create_rpath_delegate(group_id, rpath, with_block.as_ref(), is_new)
                    .await?;

                let new_service = {
                    let mut is_new = false;

                    let mut services = self.0.services.write().await;
                    let service = services
                        .entry(group_id.clone())
                        .or_insert_with(HashMap::new)
                        .entry(dec_id.clone())
                        .or_insert_with(HashMap::new)
                        .entry(rpath.to_string())
                        .or_insert_with(|| {
                            is_new = true;

                            RPathService::new(
                                GroupRPath::new(
                                    group_id.clone(),
                                    dec_id.clone(),
                                    rpath.to_string(),
                                ),
                                self.0.requestor.clone(),
                                delegate,
                                self.0.stack.clone(),
                            )
                        });

                    if is_new {
                        service.clone()
                    } else {
                        return Ok(service.clone());
                    }
                };

                new_service.start().await;
                Ok(new_service.clone())
            }
            None => Err(BuckyError::new(BuckyErrorCode::Reject, "is not service")),
        }
    }
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for GroupManager
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        let req_common = &param.request.common;
        let obj = &param.request.object;

        log::debug!(
            "group-command handle, level = {:?}, zone = {:?}, local-zone = {:?}, dec-id = {:?}, obj_type = {:?}",
            req_common.level,
            req_common.source.zone,
            self.0.local_zone,
            self.0.stack.dec_id(),
            obj.object.as_ref().map(|o| o.obj_type())
        );

        if !req_common.source.zone.is_current_zone()
            || self.0.local_zone.is_none()
            // || req_common.source.zone.zone != self.0.local_zone
            || self.0.stack.dec_id().is_none()
        {
            log::warn!(
                "there should no group-command from other zone, level = {:?}, zone = {:?}, local-zone = {:?}, dec-id = {:?}, obj_type = {:?}",
                req_common.level,
                req_common.source.zone,
                self.0.local_zone,
                self.0.stack.dec_id(),
                obj.object.as_ref().map(|o| o.obj_type())
            );

            return Ok(RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Pass,
                request: None,
                response: None,
            });
        }

        match obj.object.as_ref() {
            None => {
                return Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Reject,
                    request: None,
                    response: None,
                })
            }
            Some(any_obj) => {
                assert_eq!(any_obj.obj_type(), CoreObjectType::GroupCommand as u16);
                if any_obj.obj_type() != CoreObjectType::GroupCommand as u16 {
                    return Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Reject,
                        request: None,
                        response: None,
                    });
                }

                let (cmd, remain) = GroupCommand::raw_decode(obj.object_raw.as_slice())?;
                assert_eq!(remain.len(), 0);

                let resp_obj = self.on_command(cmd).await;

                let resp_cmd = resp_obj.map_or_else(
                    |err| Err(err),
                    |resp_obj| {
                        resp_obj.map_or(Ok(None), |resp_cmd| {
                            let resp_cmd = GroupCommand::from(resp_cmd);
                            resp_cmd.to_vec().map(|buf| {
                                Some(NONObjectInfo::new(resp_cmd.desc().object_id(), buf, None))
                            })
                        })
                    },
                );

                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(resp_cmd.map(|cmd| NONPostObjectInputResponse { object: cmd })),
                })
            }
        }
    }
}
