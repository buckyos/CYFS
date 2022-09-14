use super::state_storage::*;
use crate::interface::ObjectListenerManagerRef;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_util::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, RwLock};

struct OnAppActionWatcher {
    owner: AppController,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnAppActionWatcher
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!(
            "recv app_manager action request: {}",
            param.request.object.object_id
        );

        // only req post through http-local interface is valid!
        if param.request.common.source.protocol != NONProtocol::HttpLocal {
            let msg = format!(
                "app_manager action only valid on http-local interface! req protocol={:?}",
                param.request.common.source.protocol
            );
            error!("{}", msg);

            let e = BuckyError::new(BuckyErrorCode::PermissionDenied, msg);
            return Ok(RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(Err(e)),
            });
        }

        let ret = match AppManagerAction::raw_decode(&param.request.object.object_raw) {
            Ok((action, _)) => {
                let ret = self.owner.on_app_manager_action(action).await;
                match ret {
                    Ok(_) => {
                        let resp = NONPostObjectInputResponse { object: None };

                        RouterHandlerPostObjectResult {
                            action: RouterHandlerAction::Response,
                            request: None,
                            response: Some(Ok(resp)),
                        }
                    }
                    Err(e) => RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Err(e)),
                    },
                }
            }
            Err(e) => {
                let msg = format!(
                    "decode app_manager action object error! id={}, {}",
                    param.request.object.object_id, e
                );
                error!("{}", msg);

                let e = BuckyError::new(BuckyErrorCode::InvalidData, msg);
                RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(Err(e)),
                }
            }
        };

        Ok(ret)
    }
}

#[derive(Clone)]
pub(crate) struct AuthenticatedAppList {
    gateway_ip: Arc<RwLock<Option<String>>>,
    list: Arc<RwLock<HashMap<String, DecIpInfo>>>,
    storage: Arc<AppLocalStateStorage>,
}

impl AuthenticatedAppList {
    fn new(config_isolate: Option<String>) -> Self {
        Self {
            gateway_ip: Arc::new(RwLock::new(None)),
            list: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(AppLocalStateStorage::new(config_isolate)),
        }
    }

    pub(crate) async fn load(&self) -> BuckyResult<()> {
        let data = self.storage.load().await?;
        if data.is_none() {
            return Ok(());
        }

        let data = data.unwrap();
        if let Some(gateway_ip) = data.gateway_ip {
            info!("load app auth gateway ip: {}", gateway_ip);
            *self.gateway_ip.write().unwrap() = Some(gateway_ip);
        }

        for item in data.list {
            let dec_info = DecIpInfo {
                name: item.name,
                ip: item.ip,
            };
            self.register(&item.dec_id, &dec_info);
        }

        Ok(())
    }

    pub(crate) async fn save(&self) -> BuckyResult<()> {
        let data = self.dump_data();
        self.storage.save(data).await
    }

    fn dump_data(&self) -> AppListSavedData {
        let list = self.list.read().unwrap();
        let list = list
            .iter()
            .map(|(dec_id, dec_info)| AppLocalStateSavedData {
                dec_id: dec_id.to_owned(),
                name: dec_info.name.clone(),
                ip: dec_info.ip.clone(),
            })
            .collect();

        let gateway_ip = self.gateway_ip.read().unwrap().clone();
        AppListSavedData { gateway_ip, list }
    }

    pub fn count(&self) -> usize {
        self.list.read().unwrap().len()
    }

    pub fn gateway_ip(&self) -> Option<String> {
        self.gateway_ip.read().unwrap().clone()
    }

    fn set_gateway_ip(&self, docker_gateway_ip: &str) {
        *self.gateway_ip.write().unwrap() = Some(docker_gateway_ip.to_owned());
    }

    fn register(&self, dec_id: &str, dec_info: &DecIpInfo) {
        let mut list = self.list.write().unwrap();
        match list.entry(dec_id.to_owned()) {
            Entry::Vacant(v) => {
                info!(
                    "register authenticated app: dec_id={}, dec_info={:?}",
                    dec_id, dec_info
                );
                v.insert(dec_info.to_owned());
            }
            Entry::Occupied(mut o) => {
                warn!("register authenticated app but already exists! now will replace, dec_id={}, old dec_info={:?}, new dec_info={:?}", 
                    dec_id, o.get(), dec_info);
                o.insert(dec_info.to_owned());
            }
        }
    }

    fn unregister(&self, dec_id: &str) -> usize {
        let mut list = self.list.write().unwrap();
        match list.remove(dec_id) {
            Some(dec_info) => {
                info!(
                    "unregister authenticated app: dec_id={}, dec_info={:?}",
                    dec_id, dec_info
                );
            }
            None => {
                info!(
                    "unregister authenticated app but not found: dec_id={}",
                    dec_id
                );
            }
        }

        list.len()
    }

    pub fn check_auth(&self, dec_id: &str, addr: &str) -> BuckyResult<()> {
        let list = self.list.read().unwrap();
        match list.get(dec_id) {
            Some(dec_info) => {
                if dec_info.ip == addr {
                    Ok(())
                } else {
                    let msg = format!(
                        "app auth info not match! dec_id={}, addr={}, register addr={}",
                        dec_id, addr, dec_info.ip
                    );
                    error!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                }
            }
            None => {
                let msg = format!("app auth info not found! dec_id={}, addr={}", dec_id, addr);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        }
    }
}

// manage all app/dec's register/unresiter actions
#[derive(Clone)]
pub(crate) struct AppController {
    auth_app_list: AuthenticatedAppList,
    listener_manager: ObjectListenerManagerRef,
}

const APP_MANAGER_CONTROLLER_HANDLER_ID: &str = "system_app_manager_controller";

impl AppController {
    pub fn new(config_isolate: Option<String>, listener_manager: ObjectListenerManagerRef) -> Self {
        Self {
            listener_manager,
            auth_app_list: AuthenticatedAppList::new(config_isolate),
        }
    }

    pub async fn init(
        &self,
        router_handlers: &RouterHandlerManagerProcessorRef,
    ) -> BuckyResult<()> {
        let filter = format!("obj_type == {}", CoreObjectType::AppManagerAction.as_u16(),);

        // add post_object handler for app_manager's action cmd
        let routine = OnAppActionWatcher {
            owner: self.clone(),
        };

        if let Err(e) = router_handlers
            .post_object()
            .add_handler(
                RouterHandlerChain::Handler,
                APP_MANAGER_CONTROLLER_HANDLER_ID,
                1,
                &filter,
                RouterHandlerAction::Reject,
                Some(Box::new(routine)),
            )
            .await
        {
            error!("add app_manager controller handler error! {}", e);
            return Err(e);
        }

        info!("add app_manager controller success! filter={}", filter);

        if let Ok(_) = self.auth_app_list.load().await {
            if self.auth_app_list.count() > 0 {
                if let Some(docker_gateway_ip) = self.auth_app_list.gateway_ip() {
                    if let Err(e) = self
                        .listener_manager
                        .start_authenticated_interface(
                            &docker_gateway_ip,
                            self.auth_app_list.clone(),
                        )
                        .await
                    {
                        error!(
                            "start authenticated interface on startup error! gateway_ip={}, {}",
                            docker_gateway_ip, e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn on_app_manager_action(&self, action: AppManagerAction) -> BuckyResult<()> {
        match action.action() {
            AppManagerActionEnum::RegisterDec(action) => {
                for (dec_id, dec_info) in &action.dec_list {
                    self.auth_app_list.register(&dec_id, dec_info);
                }

                self.auth_app_list.set_gateway_ip(&action.docker_gateway_ip);

                let _ = self.auth_app_list.save().await;

                if self.auth_app_list.count() > 0 {
                    self.listener_manager
                        .start_authenticated_interface(
                            &action.docker_gateway_ip,
                            self.auth_app_list.clone(),
                        )
                        .await?;
                }
            }
            AppManagerActionEnum::UnregisterDec(action) => {
                let mut count = 1;
                for (dec_id, _info) in &action.dec_list {
                    count = self.auth_app_list.unregister(&dec_id);
                }

                let _ = self.auth_app_list.save().await;

                // will stop interface if empty
                if count == 0 {
                    info!("now will stop authenticated interface...");
                    let _ = self.listener_manager.stop_authenticated_interface().await;
                }
            }
            AppManagerActionEnum::ModifyAcl(_action) => {
                todo!();
            }
        }

        Ok(())
    }
}
