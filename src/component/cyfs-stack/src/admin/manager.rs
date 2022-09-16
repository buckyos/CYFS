use crate::config::StackGlobalConfig;
use crate::crypto_api::*;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneRoleManager;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;

use std::sync::Arc;

struct OnAdminCommandWatcher {
    owner: AdminManager,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnAdminCommandWatcher
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!(
            "recv admin command request: {}",
            param.request.object.object_id
        );

        let ret = match self
            .owner
            .on_admin_command(&param.request.common.source, &param.request.object)
            .await
        {
            Ok(()) => Ok(NONPostObjectInputResponse {
                object: None,
            }),
            Err(e) => Err(e),
        };

        let resp = RouterHandlerPostObjectResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: Some(ret),
        };

        Ok(resp)
    }
}

const ADMIN_MANAGER_HANDLER_ID: &str = "system_admin_manager_command_controller";

#[derive(Clone)]
pub struct AdminManager {
    role_manager: ZoneRoleManager,
    obj_verifier: Arc<ObjectVerifier>,
    config: StackGlobalConfig,
}

impl AdminManager {
    pub(crate) fn new(
        role_manager: ZoneRoleManager,
        obj_verifier: Arc<ObjectVerifier>,
        config: StackGlobalConfig,
    ) -> Self {
        Self {
            role_manager,
            obj_verifier,
            config,
        }
    }

    pub async fn init(
        &self,
        router_handlers: &RouterHandlersManager,
    ) -> BuckyResult<()> {
        self.register_router_handler(router_handlers).await?;

        Ok(())
    }

    async fn register_router_handler(
        &self,
        router_handlers: &RouterHandlersManager,
    ) -> BuckyResult<()> {
        let filter = format!("obj_type == {}", CoreObjectType::Admin as u16);

        // add post_object handler for app_manager's action cmd
        let routine = OnAdminCommandWatcher {
            owner: self.clone(),
        };

        if let Err(e) = router_handlers
            .post_object()
            .add_handler(
                RouterHandlerChain::Handler,
                ADMIN_MANAGER_HANDLER_ID,
                1,
                Some(filter),
                None,
                RouterHandlerAction::Default,
                Some(Box::new(routine)),
            )
            .await
        {
            error!("add admin_manager command handler error! {}", e);
            return Err(e);
        }

        Ok(())
    }

    async fn on_admin_command(&self, source: &RequestSourceInfo, object: &NONObjectInfo) -> BuckyResult<()> {
        // decode to AdminObject
        let admin_object = AdminObject::clone_from_slice(&object.object_raw).map_err(|e| {
            let msg = format!(
                "invalid admin command object buffer! id={}, {}",
                object.object_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // verify target is current device
        let zone_info = self.role_manager.zone_manager().get_current_info().await?;
        if zone_info.device_id != *admin_object.target() {
            let msg = format!(
                "unmatch admin command object target! target={}, current={}",
                admin_object.target(),
                zone_info.device_id
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        // check if command source'device is current zone's device
        if !source.is_current_zone() {
            let msg = format!(
                "command source device is not in current zone! device={}",
                source,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        // check if desc has people's sign
        self.verfiy_people_signs(&object.object_id, object.object.as_ref().unwrap())
            .await.map_err(|e| {
                let msg = format!(
                    "invalid people's signature! id={}, {}",
                    object.object_id, e,
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidSignature, msg)
            })?;

        let cmd = admin_object.into_command();
        self.process_command(cmd).await
    }

    async fn verfiy_people_signs(
        &self,
        object_id: &ObjectId,
        object: &Arc<AnyNamedObject>,
    ) -> BuckyResult<()> {
        let zone_info = self.role_manager.zone_manager().get_current_info().await?;
        let people_id = zone_info.owner.object_id();
        let people = NONSlimObjectInfo::new(people_id.clone(), None, Some(zone_info.owner.clone()));

        let req = VerifyObjectInnerRequest {
            sign_type: VerifySignType::Desc,
            object: ObjectInfo {
                object_id: object_id.to_owned(),
                object: object.clone(),
            },
            sign_object: VerifyObjectType::Object(people),
        };

        match self.obj_verifier.verify_object_inner(req).await {
            Ok(ret) => {
                if ret.valid {
                    info!(
                        "verify object people's desc sign success! id={}, people={}",
                        object_id, people_id
                    );
                    Ok(())
                } else {
                    let msg = format!(
                        "verify object people's desc sign unmatch! id={}, people={}",
                        object_id, people_id
                    );
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg))
                }
            }
            Err(e) => {
                error!(
                    "verify object people's desc sign error! id={}, people={}, {}",
                    object_id, people_id, e
                );
                Err(e)
            }
        }
    }

    async fn process_command(&self, cmd: AdminCommand) -> BuckyResult<()> {
        match cmd {
            AdminCommand::GlobalStateAccessMode(access_mode) => {
                self.process_access_mode(access_mode).await
            }
        }
    }

    async fn process_access_mode(
        &self,
        access_mode: AdminGlobalStateAccessModeData,
    ) -> BuckyResult<()> {
        self.config.change_access_mode(access_mode.category, access_mode.access_mode);
        Ok(())
    }
}
