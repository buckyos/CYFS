use crate::app_cmd_executor::AppCmdExecutor;
use crate::app_controller::AppController;
use crate::app_install_detail::AppInstallDetail;
use crate::event_handler::EventListener;
use crate::non_helper::*;
use async_std::channel::{Receiver, Sender};
use async_std::stream::StreamExt;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use log::*;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use version_compare::Version;
use app_manager_lib::{AppManagerConfig, AppSource};

//pub const USER_APP_LIST: &str = "user_app";

//中间状态最长保持时间:15分钟
const STATUS_LASTED_TIME_LIMIT_IN_MICROS: u64 = 15 * 60 * 1000 * 1000;

//1分钟检查一次状态
const CHECK_STATUS_INTERVAL_IN_SECS: u64 = 1 * 60; //2 * 60 * 1000 * 1000;
                                                   //每6小时检查一次app的新版本
const CHECK_APP_UPDATE_INTERVAL_IN_SECS: u64 = 6 * 60 * 60;
//get sys app list every 30 mins
const CHECK_SYS_APP_INTERVAL_IN_SECS: u64 = 30 * 60;
//sys app start retry count limit
const START_RETRY_LIMIT: u8 = 3;

/*
appmanager内部有一个执行器任务，每次被唤醒都会去清空cmd_list里面待处理的命令。
cmd的处理是通过channel串行了。所以同一时刻只有一个cmd在处理。
后续优化：如果cmd操作的是不同的app，可以同时执行
*/

pub struct AppManager {
    shared_stack: SharedCyfsStack,
    app_local_list: Arc<RwLock<Option<AppLocalList>>>,
    //sys app will install when app manager init
    sys_app_list: RwLock<Option<AppList>>,
    cmd_list: Option<Arc<Mutex<AppCmdList>>>,
    status_list: Arc<RwLock<HashMap<DecAppId, Arc<Mutex<AppLocalStatus>>>>>,
    owner: ObjectId,
    app_controller: Arc<AppController>,
    sender: Sender<bool>,
    receiver: Receiver<bool>,
    cmd_executor: Option<AppCmdExecutor>,
    non_helper: Arc<NonHelper>,
    config: AppManagerConfig,
    start_couter: Arc<RwLock<HashMap<DecAppId, u8>>>,
}

impl AppManager {
    pub fn new(shared_stack: SharedCyfsStack, config: AppManagerConfig) -> Self {
        let device = shared_stack.local_device();
        let owner = device
            .desc()
            .owner()
            .to_owned()
            .unwrap_or_else(|| device.desc().calculate_id());
        let (sender, receiver) = async_std::channel::unbounded();

        Self {
            shared_stack: shared_stack.clone(),
            app_local_list: Arc::new(RwLock::new(None)),
            sys_app_list: RwLock::new(None),
            cmd_list: None,
            status_list: Arc::new(RwLock::new(HashMap::new())),
            owner,
            app_controller: Arc::new(AppController::new(config.clone(), owner.clone())),
            sender,
            receiver,
            cmd_executor: None,
            non_helper: Arc::new(NonHelper::new(owner, shared_stack)),
            config,
            start_couter: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /*初始化时，恢复两个列表，cmd列表可能不为空（上次异常退出，没来得及执行完）*/
    pub async fn init(&mut self) -> BuckyResult<()> {
        let cmd_list = self.non_helper.get_cmd_list_obj().await;
        let cmd_list = Arc::new(Mutex::new(cmd_list));
        self.cmd_list = Some(cmd_list.clone());
        let app_local_list = self.non_helper.get_app_local_list().await;

        //根据app_local_list恢复status_list
        let mut status_list = HashMap::new();
        for app_id in app_local_list.app_list() {
            let local_status = self.non_helper.get_local_status(app_id).await;
            info!("[INIT] push status to list:{}", local_status.output());
            status_list.insert(app_id.clone(), Arc::new(Mutex::new(local_status)));
        }

        *self.app_local_list.write().unwrap() = Some(app_local_list);
        *self.status_list.write().unwrap() = status_list;

        self.app_controller.prepare_start(self.shared_stack.clone()).await?;
        AppController::start_monitor_sn(self.app_controller.clone()).await;

        self.cmd_executor = Some(AppCmdExecutor::new(
            self.owner.clone(),
            self.app_controller.clone(),
            //self.app_local_list.clone(),
            self.status_list.clone(),
            cmd_list,
            self.non_helper.clone(),
            self.config.clone()
        ));

        self.cmd_executor.as_ref().unwrap().init()
    }

    pub async fn start(manager: Arc<AppManager>) {
        let listener = EventListener {
            app_manager: manager.clone(),
        };

        //let filter = format!("obj_type == {}", CoreObjectType::AppCmd as u16);
        manager
            .shared_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "app_manager_cmd_handler",
                0,
                None, //Some(filter.clone()),
                Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                RouterHandlerAction::Default,
                Some(Box::new(listener)),
            )
            .unwrap();

        //起一个执行器，根据被唤醒的标志位进行对应操作
        let manager_executor = manager.clone();
        async_std::task::spawn(async move {
            info!("start cmd executor!");
            loop {
                if let Ok(_from_checker) = manager_executor.receiver.recv().await {
                    info!("executor awake!");
                    manager_executor
                        .cmd_executor
                        .as_ref()
                        .unwrap()
                        .execute_cmd()
                        .await;
                } else {
                    error!("executor recv failed!");
                }
            }
        });

        // 起一个1分钟的timer，检查App的状态
        let manager_checker = manager.clone();
        async_std::task::spawn(async move {
            manager_checker.check_app_status_on_startup().await;
            let mut interval =
                async_std::stream::interval(Duration::from_secs(CHECK_STATUS_INTERVAL_IN_SECS));
            while let Some(_) = interval.next().await {
                if let Err(e) = manager_checker.sender.send(false).await {
                    error!("active executor failed! err:{}", e);
                }
                manager_checker.check_app_status().await;
            }
        });

        let manager_clone = manager.clone();
        async_std::task::spawn(async move {
            info!("start update check!");
            let mut interval =
                async_std::stream::interval(Duration::from_secs(CHECK_APP_UPDATE_INTERVAL_IN_SECS));
            while let Some(_) = interval.next().await {
                manager_clone.check_app_update().await;
            }
        });

        //定期检查sysApp的更新
        let manager_sys_app = manager.clone();
        async_std::task::spawn(async move {
            info!("start install sys app!");
            manager_sys_app.install_sys_app().await;
            let mut interval =
                async_std::stream::interval(Duration::from_secs(CHECK_SYS_APP_INTERVAL_IN_SECS));
            while let Some(_) = interval.next().await {
                manager_sys_app.install_sys_app().await;
            }
        });

        // let version = manager.get_stack_version().await.unwrap();
        // info!("get stack version:{:?}", version);
    }

    //该函数判断当前命令可不可以被执行，如果可以则进入cmdList，如果不行则回复错误给用户
    //from_user表示是否该操作来自与用户
    pub async fn on_app_cmd(&self, cmd: AppCmd, from_user: bool) -> BuckyResult<()> {
        let app_id = cmd.app_id();
        let cmd_code = cmd.cmd();

        let ret;
        //添加，需要当前App不存在
        if let CmdCode::Add(add_app) = cmd_code {
            // 如果app来源是system，不允许添加App
            if self.config.app.source == AppSource::System {
                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, "disallow add app when use system app source"));
            }
            // 如果app在exclude里，不允许添加App
            if self.config.app.exclude.contains(app_id) {
                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, format!("app {} in exclude list", app_id)));
            }
            if let Some(owner_id) = add_app.app_owner_id {
                let _ = self
                    .non_helper
                    .get_dec_app(app_id.object_id(), Some(owner_id))
                    .await;
            }
            ret = self.on_add_cmd(app_id).await;
        } else {
            let status_list = self.status_list.read().unwrap().clone();
            let app_status = status_list.get(app_id);

            ret = match app_status {
                Some(status) => match cmd_code {
                    CmdCode::Remove => self.on_remove_cmd(app_id, status.clone()).await,
                    CmdCode::Install(_) | CmdCode::Uninstall | CmdCode::Start | CmdCode::Stop => {
                        self.on_common_cmd(app_id, status.clone(), cmd.clone(), from_user)
                            .await
                    }
                    CmdCode::SetPermission(_) => {
                        self.on_set_permission_cmd(app_id, status.clone(), cmd.clone())
                            .await
                    }
                    CmdCode::SetQuota(_) => {
                        self.on_set_quota_cmd(app_id, status.clone(), cmd.clone())
                            .await
                    }
                    CmdCode::SetAutoUpdate(_) => {
                        self.on_set_auto_update_cmd(app_id, status.clone(), cmd.clone())
                            .await
                    }
                    _ => Err(BuckyError::from((
                        BuckyErrorCode::Unknown,
                        "unknown AppCmd".to_string(),
                    ))),
                },
                None => {
                    warn!("app status not found in list, app:{}", app_id);
                    Err(BuckyError::from((
                        BuckyErrorCode::NotFound,
                        "app status not found in list".to_string(),
                    )))
                }
            }
        }

        ret
    }

    async fn check_app_update(&self) {
        //更新只是尽力更新，如果中间出错就等下次更新。
        info!("###### will check app update!");
        let status_list = self.status_list.read().unwrap().clone();
        for (app_id, status) in status_list {
            let status_code;
            let app_cur_version;
            {
                let status = status.lock().unwrap();
                if !status.auto_update() {
                    //auto update is turned off
                    info!("app auto update is turned off. skip it, app:{}", app_id);
                    continue;
                }
                status_code = status.status();
                let version = status.version();
                if version.is_none() {
                    continue;
                }
                app_cur_version = version.unwrap().to_owned();
            }
            //仅仅已安装的状态才做更新检查
            let can_update_status = [
                AppLocalStatusCode::NoService,
                AppLocalStatusCode::Stop,
                AppLocalStatusCode::StopFailed,
                AppLocalStatusCode::Running,
                AppLocalStatusCode::StartFailed,
                AppLocalStatusCode::RunException,
                AppLocalStatusCode::UninstallFailed,
            ];
            if !can_update_status.contains(&status_code) {
                info!(
                    "app can not update right now, app:{}, status:{}",
                    app_id, status_code
                );
                continue;
            }

            match self.get_app_update_version(&app_id, &app_cur_version).await {
                Ok(update_version) => {
                    info!(
                        "will update app, appid:{}, target version:{}",
                        app_id, update_version
                    );
                    //模拟用户发起一个安装请求
                    let install_cmd = AppCmd::install(
                        self.owner.clone(),
                        app_id,
                        &update_version,
                        status_code == AppLocalStatusCode::Running,
                    );
                    let _ = self.on_app_cmd(install_cmd, false).await;
                }
                Err(e) => {
                    info!("get app update version failed. app:{}, err: {}", app_id, e)
                }
            }
        }
    }

    async fn fix_status_on_startup(&self) {
        let status_list = self.status_list.read().unwrap().clone();
        for (app_id, status) in status_list {
            let mut status_clone = None;
            {
                let mut status = status.lock().unwrap();
                let status_code = status.status();
                let fix_status = match status_code {
                    AppLocalStatusCode::Stopping => Some(AppLocalStatusCode::StopFailed),
                    AppLocalStatusCode::Starting => Some(AppLocalStatusCode::StartFailed),
                    AppLocalStatusCode::Installing => Some(AppLocalStatusCode::InstallFailed),
                    AppLocalStatusCode::Uninstalling => Some(AppLocalStatusCode::UninstallFailed),
                    _ => None,
                };
                if fix_status.is_some() {
                    let fix_code = fix_status.unwrap();
                    status.set_status(fix_code);
                    status_clone = Some(status.clone());
                    info!(
                        "### fix app status on startup, app:{}, from {} to {}",
                        app_id, status_code, fix_code
                    );
                }
            }
            if let Some(new_status) = status_clone {
                let _ = self.non_helper.put_local_status(&new_status).await;
            }
        }
    }

    /*在appmanager启动的时候调用，会根据localstatus尝试重建本地状态
     */
    async fn check_app_status_on_startup(&self) {
        info!("######[START] check app status on startup!");
        self.fix_status_on_startup().await;

        let status_list = self.status_list.read().unwrap().clone();
        for (app_id, status) in status_list {
            let status_code = status.lock().unwrap().status();
            info!("### app:{}, status should be: {}", app_id, status_code);

            let mut need_start = false;
            let mut need_install = false;

            match status_code {
                AppLocalStatusCode::Init
                | AppLocalStatusCode::Uninstalled
                | AppLocalStatusCode::InstallFailed
                | AppLocalStatusCode::ErrStatus => {
                    info!(
                        "### [STARTUP CEHCK] pass on startup, app:{}, status:{}",
                        app_id, status_code
                    );
                    continue;
                }
                AppLocalStatusCode::Running
                | AppLocalStatusCode::StartFailed
                | AppLocalStatusCode::RunException => {
                    need_start = true;
                    need_install = true;
                }
                AppLocalStatusCode::Stop
                | AppLocalStatusCode::NoService
                | AppLocalStatusCode::StopFailed => {
                    need_install = true;
                }
                AppLocalStatusCode::UninstallFailed => {
                    //do uninstall
                    info!("### [STARTUP CEHCK] app is uninstallFailed, try to uninstall it again, app:{}", app_id);
                    let uninstall_cmd = AppCmd::uninstall(self.owner.clone(), app_id.clone());
                    let _ = self.on_app_cmd(uninstall_cmd, false).await;
                }
                v @ _ => {
                    info!(
                        "### [STARTUP CEHCK] status will not be handled!, app:{}, status:{}",
                        app_id, v
                    );
                }
            }

            if need_install || need_start {
                let target_version = status.lock().unwrap().version().map(|s| s.to_owned());
                let install_detail = AppInstallDetail::new(&app_id);
                let installed_version = install_detail.get_install_version().map(|s| s.to_owned());
                if target_version.is_some() {
                    let target_ver = target_version.unwrap();
                    info!("### [STARTUP CEHCK] app:{}, version in status:{:?}, installed version:{:?}", app_id, target_ver, installed_version);
                    if installed_version.is_none() || installed_version.unwrap() != target_ver {
                        info!("### [STARTUP CEHCK] app need install, app:{}, status:{}, ver:{:?}, need start:{}", app_id, status_code, target_ver, need_start);
                        let install_cmd = AppCmd::install(
                            self.owner.clone(),
                            app_id.clone(),
                            &target_ver,
                            need_start,
                        );
                        let _ = self.on_app_cmd(install_cmd, false).await;
                    } else if need_start {
                        info!(
                            "### [STARTUP CEHCK] app need restart, app:{}, status:{}",
                            app_id, status_code
                        );
                        let _ = self.restart_app(&app_id, status.clone()).await;
                    }
                } else {
                    unreachable!();
                }
            }
        }

        info!("######[END] check app status on startup!");
    }

    /* 根据local_status检查app状态
    已经入错误状态的app不用管。等下一个命令纠正它。
    已经入Running状态的app要确保它正在运行。
    除Running状态的其他非中间状态，不用管。
    */
    async fn check_app_status(&self) {
        info!("###### will check app status!");
        let status_list = self.status_list.read().unwrap().clone();
        for (app_id, status) in status_list {
            let status_code = status.lock().unwrap().status();
            info!(
                "###[STATUS CHECK] app:{}, status should be: {}",
                app_id, status_code
            );
            if status_code == AppLocalStatusCode::Running {
                //进入running说明启动成功，检查一下服务在不在，不在的话算运行异常
                //这里考虑重启ood以后，已经处于running状态的app，如果正在运行，要重启一次，如果没运行，要拉起一次
                self.check_running_app(&app_id, status.clone()).await;
                continue;
            }
        }
    }

    async fn install_sys_app(&self) {
        self.get_sys_app_list().await;
        info!("###### will install sys apps!");
        let sys_app_list = self.sys_app_list.read().unwrap().clone();
        if sys_app_list.is_none() {
            info!("sys app list is empty! skip.");
            return;
        }
        let sys_app_list = sys_app_list.unwrap();
        let status_list = self.status_list.read().unwrap().clone();
        let sys_app_list = sys_app_list.app_list();

        //if app not installed, install it
        //if app install failed or start failed, retry. only if installed version == target version
        for (app_id, target_status) in sys_app_list {
            let target_version = target_status.version();
            let target_status = target_status.status();
            info!(
                "### sys app :{}, target status:{}, target version:{}",
                app_id, target_status, target_version
            );
            if !target_status {
                info!("### sys app not need to be installed, skip, app:{}", app_id);
                continue;
            }
            let local_status = status_list.get(app_id);
            let mut need_install = false;
            //let mut reset_retry = false;
            let installed_version;
            let status_code;

            if let Some(local_status) = local_status {
                let local_status = local_status.lock().unwrap();
                info!("### sys app status: {}", local_status.output());
                status_code = local_status.status();
                installed_version = local_status.version().map_or(None, |v| Some(v.to_owned()));
            } else {
                info!("### sys app status is not found. app:{}", app_id);
                //list里没有，直接取一次status
                let local_status = self.non_helper.get_local_status(app_id).await;
                status_code = local_status.status();
                installed_version = local_status.version().map_or(None, |v| Some(v.to_owned()));
            }

            if status_code == AppLocalStatusCode::Init {
                info!("### sys app is inited, will install it, app:{}", app_id);
                need_install = true;
            } else if status_code == AppLocalStatusCode::InstallFailed
                || status_code == AppLocalStatusCode::StartFailed
            {
                if let Some(ver) = installed_version {
                    if ver == target_version {
                        info!(
                            "### sys app did not start correctly, will reinstall it, app:{}",
                            app_id
                        );
                        need_install = true
                    }
                }
            }

            if !need_install {
                continue;
            }

            {
                let mut counters = self.start_couter.write().unwrap();

                let cur_count = *counters.get(app_id).unwrap_or(&0);
                if cur_count > START_RETRY_LIMIT {
                    info!(
                        "###sys app start retry count is out of limit! skip it. app:{}",
                        app_id
                    );
                    continue;
                } else {
                    info!(
                        "### start sys app:{}, retry count:{}",
                        app_id,
                        cur_count + 1
                    );
                    counters.insert(app_id.clone(), cur_count + 1);
                }
            }

            info!(
                "### will install sys app:{}, ver:{}",
                app_id, target_version
            );
            // simulate user request, after install, will manager the app as a user app.
            let _ = self.on_add_cmd(app_id).await;
            let install_cmd =
                AppCmd::install(self.owner.clone(), app_id.clone(), target_version, true);
            let _ = self.on_app_cmd(install_cmd, false).await;
        }
    }

    async fn check_running_app(&self, app_id: &DecAppId, status: Arc<Mutex<AppLocalStatus>>) {
        match self
            .app_controller
            .is_app_running(app_id)
            .await
        {
            Ok(is_running) => {
                info!("[RUNNING CHECK] running: [{}] app:{}", is_running, app_id);
                if is_running {
                    let mut writer = self.start_couter.write().unwrap();
                    let running_counter = writer.entry(app_id.clone()).or_insert(0);
                    if *running_counter > 0 {
                        info!("reset app {} restart counter {}", app_id, *running_counter);
                        *running_counter = 0;
                    }
                    return;
                } else {
                    let mut try_start = false;
                    let mut status_clone = None;
                    {
                        let mut status = status.lock().unwrap();
                        let cur_status_code = status.status();
                        if cur_status_code != AppLocalStatusCode::Running {
                            //判断状态是否还是Running，如果不是就不改变状态了
                            info!(
                            "[RUNNING CHECK] after check app running, but current status is not running, skip. app:{}, status: {}",
                            app_id, cur_status_code
                        );
                            return;
                        }
                        //status is running, but not actually
                        let mut writer = self.start_couter.write().unwrap();
                        let running_counter = writer.entry(app_id.clone()).or_insert(0);
                        if *running_counter > START_RETRY_LIMIT {
                            let target_status_code = AppLocalStatusCode::RunException;
                            info!("[RUNNING CHECK] app failed count is out of limit! app:{}, change app status from [{}] to [{}]", 
                            app_id, cur_status_code, target_status_code);
                            status.set_status(target_status_code);
                            status_clone = Some(status.clone());
                        } else {
                            *running_counter = *running_counter + 1;
                            info!("[RUNNING CHECK] app status is running, but not actually. will restart it, app:{}, retry count:{}", app_id, *running_counter);
                            try_start = true;
                        }
                    }
                    if try_start {
                        let _ = self.restart_app(app_id, status.clone()).await;
                    } else if let Some(new_status) = status_clone {
                        let _ = self.non_helper.put_local_status(&new_status).await;
                    }
                }
            }
            Err(e) => {
                warn!(
                    "[RUNNING CHECK] checking running status failed will reinstall it, app:{}, err: {}",
                    app_id, e
                );
                let version = status.lock().unwrap().version().unwrap().to_owned();
                let _ = self.on_add_cmd(app_id).await;
                let install_cmd =
                    AppCmd::install(self.owner.clone(), app_id.clone(), &version, true);
                let _ = self.on_app_cmd(install_cmd, false).await;
            }
        }
    }

    //这一组函数的意义是响应cmd事件，判断是否可以执行cmd，如果可以执行，改变local_status并且将cmd加入队列
    async fn on_add_cmd(&self, app_id: &DecAppId) -> BuckyResult<()> {
        info!("recv add cmd, app:{}", app_id);
        // 如果app在exclude里，不允许添加App
        if self.config.app.exclude.contains(app_id) {
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, format!("app {} in exclude list", app_id)));
        }
        if self
            .app_local_list
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .exists(app_id)
        {
            let err_msg = format!("app already exists, app:{}", app_id);
            warn!("{}", err_msg);
            return Err(BuckyError::from((BuckyErrorCode::AlreadyExists, err_msg)));
        } else {
            //add直接添加，不用加入到cmd队列。添加App到appList
            info!("add app to list, app:{}", app_id);
            self.app_local_list
                .write()
                .unwrap()
                .as_mut()
                .unwrap()
                .insert(app_id.clone());

            let clone_app_list = self
                .app_local_list
                .read()
                .unwrap()
                .as_ref()
                .unwrap()
                .clone();
            let _ = self.non_helper.put_app_local_list(&clone_app_list).await;

            //创建新的status并put
            let new_status = self.non_helper.get_local_status(app_id).await;
            self.status_list
                .write()
                .unwrap()
                .insert(app_id.clone(), Arc::new(Mutex::new(new_status)));
        }
        Ok(())
    }

    async fn on_remove_cmd(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
    ) -> BuckyResult<()> {
        info!("recv remove cmd, app:{}", app_id);
        {
            let status = status.lock().unwrap();
            let status_code = status.status();
            //可以remove的前置状态
            if !AppCmdExecutor::is_valid_pre_status(&CmdCode::Remove, status_code) {
                let err_msg = format!("cannot remove, current status is {}", status_code);
                warn!("{}", err_msg);
                return Err(BuckyError::from((BuckyErrorCode::ErrorState, err_msg)));
            }
        }

        //刚添加或者安装失败的状态可以直接remove
        self.app_local_list
            .write()
            .unwrap()
            .as_mut()
            .unwrap()
            .remove(app_id);

        let app_list = self
            .app_local_list
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .clone();
        let _ = self.non_helper.put_app_local_list(&app_list).await;

        self.status_list.write().unwrap().remove(&app_id);

        info!("app removed. {}", app_id);

        Ok(())
    }

    //接收到用户发起的通用的cmd，包括install，uninstall，start，stop。
    //from_user表示是否是来自用户的操作，如果是用户操作的start和install，需要重置retry_counter
    async fn on_common_cmd(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: AppCmd,
        from_user: bool,
    ) -> BuckyResult<()> {
        let cmd_code = cmd.cmd();
        info!("recv cmd [{:?}], app:{}", cmd_code, app_id);

        match cmd_code {
            CmdCode::Install(_) | CmdCode::Start => {
                // 如果app在exclude里，不允许App安装或启动
                if self.config.app.exclude.contains(app_id) {
                    return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, format!("app {} in exclude list", app_id)));
                }

                if from_user {
                    info!(
                        "recv cmd from user, cmd: {}, will reset retry count.",
                        cmd.output()
                    );
                    let mut counters = self.start_couter.write().unwrap();
                    counters.insert(app_id.clone(), 0);
                }
            }
            CmdCode::Uninstall | CmdCode::Stop => {}
            _ => {
                let err_msg = format!("cmd cannot processed!. {:?}", cmd_code);
                error!("{}", err_msg);
                return Err(BuckyError::from((BuckyErrorCode::InvalidInput, err_msg)));
            }
        }

        let mut cmd_group = self.get_cmd_group(&cmd);

        let status_clone;
        {
            let mut status = status.lock().unwrap();
            let status_code = status.status();

            //找到cmd_group里面，在当前状态下第一个可以执行的命令
            while cmd_group.len() > 0 {
                if AppCmdExecutor::is_valid_pre_status(cmd_group[0].0.cmd(), status_code) {
                    break;
                }
                cmd_group.pop_front();
            }

            //没有可以执行的命令
            if cmd_group.len() == 0 {
                let err_msg = format!(
                    "cannot do cmd: {}, current status is {}",
                    cmd.output(),
                    status_code
                );
                warn!("{}", err_msg);
                return Err(BuckyError::from((BuckyErrorCode::ErrorState, err_msg)));
            }

            let mut cmd_group_code = vec![];
            for (cmd, _retry_count) in &cmd_group {
                cmd_group_code.push(cmd.cmd());
            }

            //这里先设置状态，以免后续有命令来读
            let next_cmd = &cmd_group[0].0;
            let next_status_code =
                AppCmdExecutor::get_next_status_with_cmd(next_cmd.cmd()).unwrap();

            info!(
                "cmd accept [{:?}], will change status from [{}] to [{}], app:{}, cmd groups: {:?}",
                cmd_code, status_code, next_status_code, app_id, cmd_group_code
            );
            status.set_status(next_status_code);
            status_clone = status.clone();
        }

        let _ = self.non_helper.put_local_status(&status_clone).await;

        //添加Cmd进cmdlist

        let _ = self.push_cmd(&cmd_group.into()).await;

        Ok(())
    }

    //对于用户发起的命令，会被转化为一组命令的组合，一般按照最长路径去组合，然后根据当前状态判断从哪一条命令执行
    //这个主要是为了交互方便，可以根据产品要求改变翻译策略
    fn get_cmd_group(&self, cmd: &AppCmd) -> VecDeque<(AppCmd, u32)> {
        let cmd_code = cmd.cmd();
        let app_id = cmd.app_id().clone();
        let cmd_owner = cmd.desc().owner().unwrap();
        let mut cmd_group = VecDeque::new();

        match cmd_code {
            CmdCode::Install(install_app) => {
                cmd_group.push_back((AppCmd::stop(cmd_owner, app_id.clone()), 0));
                cmd_group.push_back((AppCmd::uninstall(cmd_owner, app_id.clone()), 0));
                cmd_group.push_back((cmd.clone(), 0));
                if install_app.run_after_install {
                    //install以后需要启动才start
                    cmd_group.push_back((AppCmd::start(cmd_owner, app_id.clone()), 0));
                }
            }
            CmdCode::Uninstall => {
                cmd_group.push_back((AppCmd::stop(cmd_owner, app_id.clone()), 0));
                cmd_group.push_back((cmd.clone(), 0));
            }
            _v => {
                cmd_group.push_back((cmd.clone(), 0));
            }
        }
        cmd_group
    }

    /* 理论上设置权限不要重启应用，暂时不做重启 */
    async fn on_set_permission_cmd(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: AppCmd,
    ) -> BuckyResult<()> {
        let cmd_code = cmd.cmd();
        info!("on set permission, app:{}, cmd: {:?}", app_id, cmd_code);

        let mut permissions = HashMap::new();
        if let CmdCode::SetPermission(param) = cmd_code {
            for (k, v) in &param.permission {
                let permission_state = match v {
                    true => PermissionState::Granted,
                    false => PermissionState::Blocked,
                };
                permissions.insert(k.to_string(), permission_state);
                info!(
                    "set permission for app:{}, {}: {}",
                    app_id, k, permission_state
                );
            }
        } else {
            let err_msg = format!("recv cmd: {:?}, expect set permission cmd", cmd_code);
            warn!("{}", err_msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidParam, err_msg)));
        }

        //根据权限设置具体的ACL TODO
        {
            status.lock().unwrap().set_permissions(&permissions);
            let status_clone = status.lock().unwrap().clone();
            let _ = self.non_helper.put_local_status(&status_clone).await;
        }
        Ok(())

        //添加Cmd进cmdlist，如果设置权限不需要重启，那就不用加命令
    }

    /* 设置配额这个命令比较特殊，会根据设置的时候的状态改变状态。
    在运行中，启动中，那么设置后要用新的配额参数重启。重启操作需要插队。即在队列头部插入Stop和Start命令。
    在安装中，已安装，停止中，卸载中，之类的，直接设置配额即可，下次启动会用新配额启动。*/
    async fn on_set_quota_cmd(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: AppCmd,
    ) -> BuckyResult<()> {
        let cmd_code = cmd.cmd();
        info!("on set quota, app:{}, cmd: {:?}", app_id, cmd_code);

        if let CmdCode::SetQuota(quota) = cmd_code {
            let status_code;
            let quota_changed;
            {
                let mut status = status.lock().unwrap();
                quota_changed = status.set_quota(&quota);
                status_code = status.status();
            }

            if !quota_changed {
                info!("quota is not changed, skip it. app:{}", app_id);
                return Ok(());
            }

            if status_code == AppLocalStatusCode::Running
                || status_code == AppLocalStatusCode::Starting
            {
                info!(
                    "app is {}, will restart it with new quota. app:{}",
                    status_code, app_id
                );
                let _ = self.restart_app(app_id, status.clone()).await;
            }
        } else {
            let err_msg = format!("recv cmd: {:?}, expect set permission cmd", cmd_code);
            warn!("{}", err_msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidParam, err_msg)));
        }

        Ok(())
    }

    async fn on_set_auto_update_cmd(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: AppCmd,
    ) -> BuckyResult<()> {
        let cmd_code = cmd.cmd();
        info!("on set auto update, app:{}, cmd: {:?}", app_id, cmd_code);

        if let CmdCode::SetAutoUpdate(auto_update) = cmd_code {
            let old_value = status.lock().unwrap().set_auto_update(*auto_update);
            if old_value != *auto_update {
                //if auto_update changed, then put status object
                let status_clone = status.lock().unwrap().clone();
                let _ = self.non_helper.put_local_status(&status_clone).await;
            }
        } else {
            let err_msg = format!("recv cmd: {:?}, expect set auto update cmd", cmd_code);
            warn!("{}", err_msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidParam, err_msg)));
        }

        Ok(())
    }

    // 检查所有App兼容性
    async fn check_all_app_compatibility(&self, stack_version: &str) -> BuckyResult<()> {
        // let version = self.get_stack_version().await?;
        // info!("get stack version:{:?}", version);

        //获取需要检查的列表：appid, version
        let mut check_list: Vec<(DecAppId, bool)> = vec![];
        let status_list = self.status_list.read().unwrap().clone();
        for (app_id, status) in status_list {
            let status_code;
            {
                let status = status.lock().unwrap();
                status_code = status.status();
            }

            if status_code != AppLocalStatusCode::Init
                && status_code != AppLocalStatusCode::Uninstalled
            {
                check_list.push((app_id.clone(), false));
            }
        }

        for item in check_list.iter_mut() {
            match self
                .check_app_compatibility(stack_version, &item.0)
                .await
            {
                Err(e) => {
                    info!("check app compatibility failed, err: {}", e);
                }
                Ok(pass) => {
                    info!("check app compatibility complete, pass: {}", pass);
                    item.1 = pass;
                }
            }
        }

        // info!(
        //     "###### check app compatibility, stack version: {}, result: {:?}",
        //     stack_version, check_list
        // );

        Ok(())
    }

    async fn check_app_compatibility(
        &self,
        stack_version: &str,
        app_id: &DecAppId,
    ) -> BuckyResult<bool> {
        info!(
            "will check app compatibility, stack version: {}, appId: {}",
            stack_version, app_id
        );
        let stack_ver = Version::from(stack_version).unwrap();
        let ver_dep = self
            .app_controller
            .get_app_version_dep(app_id)
            .await?;
        if ver_dep.0 != "*" {
            let min_ver = Version::from(&ver_dep.0).unwrap();
            if min_ver > stack_ver {
                return Ok(false);
            }
        }
        if ver_dep.1 != "*" {
            let max_ver = Version::from(&ver_dep.1).unwrap();
            if max_ver < stack_ver {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn get_sys_app_list_owner_id(&self) -> Option<ObjectId> {
        let mut repo_path = cyfs_util::get_cyfs_root_path();
        repo_path.push("etc");
        repo_path.push("desc");
        repo_path.push("app_repo.desc");
        if repo_path.exists() {
            match AnyNamedObject::decode_from_file(&repo_path, &mut vec![]) {
                Ok((obj, _)) => Some(obj.calculate_id()),
                Err(e) => {
                    error!("decode app_repo.desc failed, err {}", e);
                    None
                }
            }
        } else {
            debug!("not found app_repo.desc");
            None
        }
    }

    async fn get_sys_app_list(&self) {
        if self.config.app.can_install_system() {
            if let Some(id) = self.get_sys_app_list_owner_id() {
                // 得到AppId
                let sys_app_list_id = AppList::generate_id(id.clone(), "", APPLIST_APP_CATEGORY);
                info!("try get sys app list {}", sys_app_list_id);
                // 用non，从target或链上取真正的AppList
                match self
                    .non_helper
                    .get_object(&sys_app_list_id, None, CYFS_ROUTER_REQUEST_FLAG_FLUSH)
                    .await
                {
                    Ok(resp) => {
                        if let Ok(app_list) = AppList::clone_from_slice(&resp.object.object_raw) {
                            // 这里只存储，这个函数只在初始化时候调用，后续有check status的步骤
                            *self.sys_app_list.write().unwrap() = Some(app_list);
                        }
                    }
                    Err(e) => {
                        warn!("get sys app list from {} fail, err {}", &id, e);
                    }
                }
            }
        }

        // 把app include也加入sys_app_list
        {
            let mut list = self.sys_app_list.write().unwrap();
            if self.config.app.include.len() > 0 && list.is_none() {
                *list = Some(AppList::create(self.owner.clone(), "", APPLIST_APP_CATEGORY));
            }
        }

        for id in &self.config.app.include {
            if let Ok(latest_version) = self.get_app_update_version(id, "0.0.0").await {
                info!("add include app {} ver {}", id, &latest_version);
                self.sys_app_list.write().unwrap().as_mut().unwrap().put(AppStatus::create(self.owner.clone(), id.clone(), latest_version, true));
            }
        }
    }

    async fn get_stack_version(&self) -> BuckyResult<VersionInfo> {
        let req = UtilGetVersionInfoRequest::new();
        let version_info = self.shared_stack.util().get_version_info(req).await?;
        info!("get ood stack version: {:?}", version_info.info);

        Ok(version_info.info)
    }

    //获取app的可更新的最新版本
    async fn get_app_update_version(
        &self,
        app_id: &DecAppId,
        current_version: &str,
    ) -> BuckyResult<String> {
        let current_version = Version::from(current_version).unwrap();
        let mut latest_ver = current_version;
        let mut found = false;
        let dec_app = self
            .non_helper
            .get_dec_app(app_id.object_id(), None)
            .await?;
        let app_source = dec_app.source();
        for (ver, _) in app_source {
            let version = Version::from(ver).unwrap();
            if version > latest_ver {
                latest_ver = version;
                found = true;
            }
        }

        if found {
            Ok(latest_ver.as_str().to_owned())
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotFound))
        }
    }

    async fn push_cmd(&self, cmds: &Vec<(AppCmd, u32)>) -> BuckyResult<()> {
        let cmd_list_clone;
        {
            let mut cmd_list = self.cmd_list.as_ref().unwrap().lock().unwrap();
            for (cmd, retry_count) in cmds {
                cmd_list.push_back(cmd.clone(), *retry_count);
            }
            cmd_list_clone = cmd_list.clone();
        }

        info!(
            "push back {} cmds,  new cmd list is: {}",
            cmds.len(),
            cmd_list_clone.output()
        );
        let _ = self.non_helper.put_cmd_list(&cmd_list_clone).await;
        if let Err(e) = self.sender.send(false).await {
            error!("active executor failed! err:{}", e);
        }

        Ok(())
    }

    //重启app，先stop，再start
    async fn restart_app(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
    ) -> BuckyResult<()> {
        info!("will restart app:{}", app_id);
        let cmds;
        let mut status_clone = None;
        {
            let mut status = status.lock().unwrap();
            let status_code = status.status();

            //如果可以启动，直接启动即可
            //如果不能启动，则先尝试关闭再启动
            if AppCmdExecutor::is_valid_pre_status(&CmdCode::Start, status_code) {
                cmds = vec![(AppCmd::start(self.owner.clone(), app_id.clone()), 0)];
            } else {
                cmds = vec![
                    (AppCmd::stop(self.owner.clone(), app_id.clone()), 0),
                    (AppCmd::start(self.owner.clone(), app_id.clone()), 0),
                ];
            }

            //如果能设置状态就先设置状态，防止被其他命令改变
            let next_cmd = &cmds[0].0;
            if AppCmdExecutor::is_valid_pre_status(next_cmd.cmd(), status_code) {
                let next_status_code =
                    AppCmdExecutor::get_next_status_with_cmd(next_cmd.cmd()).unwrap();
                status.set_status(next_status_code);
                status_clone = Some(status.clone());

                info!(
                    "will change status from [{}] to [{}], app:{}",
                    status_code, next_status_code, app_id
                );
            }
        }

        if let Some(status_clone) = status_clone {
            let _ = self.non_helper.put_local_status(&status_clone).await;
        }

        self.push_front_cmd(&cmds).await
    }

    async fn push_front_cmd(&self, cmds: &Vec<(AppCmd, u32)>) -> BuckyResult<()> {
        let cmd_list_clone;
        {
            let mut cmd_list = self.cmd_list.as_ref().unwrap().lock().unwrap();
            for (cmd, retry_count) in cmds.iter().rev() {
                cmd_list.push_front(cmd.clone(), *retry_count);
            }
            cmd_list_clone = cmd_list.clone();
        }

        info!(
            "push front {} cmds,  new cmd list is: {}",
            cmds.len(),
            cmd_list_clone.output()
        );
        let _ = self.non_helper.put_cmd_list(&cmd_list_clone).await;
        if let Err(e) = self.sender.send(false).await {
            error!("active executor failed! err:{}", e);
        }

        Ok(())
    }
}
