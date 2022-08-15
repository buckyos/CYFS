use crate::app_controller::{AppActionResult, AppController};
use crate::app_install_detail::AppInstallDetail;
use crate::docker_api::*;
use crate::docker_network_manager::{DockerNetworkManager, CYFS_BRIDGE_NAME};
use crate::non_helper::*;
use cyfs_base::*;
use cyfs_core::*;
use log::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

const APP_DIR_MAIN_PATH: &str = "/app";

pub struct AppCmdExecutor {
    owner: ObjectId,
    app_controller: Arc<AppController>,
    docker_network_manager: DockerNetworkManager,
    //app_local_list: Arc<RwLock<Option<AppLocalList>>>,
    status_list: Arc<RwLock<HashMap<DecAppId, Arc<Mutex<AppLocalStatus>>>>>,
    cmd_list: Arc<Mutex<AppCmdList>>,
    non_helper: Arc<NonHelper>,
    use_docker: bool,
}

impl AppCmdExecutor {
    pub fn new(
        owner: ObjectId,
        app_controller: Arc<AppController>,
        //app_local_list: Arc<RwLock<Option<AppLocalList>>>,
        status_list: Arc<RwLock<HashMap<DecAppId, Arc<Mutex<AppLocalStatus>>>>>,
        cmd_list: Arc<Mutex<AppCmdList>>,
        non_helper: Arc<NonHelper>,
        use_docker: bool,
    ) -> Self {
        Self {
            owner,
            app_controller,
            //app_local_list,
            docker_network_manager: DockerNetworkManager::new(),
            status_list,
            cmd_list,
            non_helper,
            use_docker,
        }
    }

    pub fn init(&self) -> BuckyResult<()> {
        if self.use_docker {
            if let Err(e) = self.docker_network_manager.init() {
                error!("init docker network manager failed, err: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    pub async fn execute_cmd(&self) {
        loop {
            let cmd_item;
            let cmd_list_clone;
            {
                let mut cmd_list = self.cmd_list.lock().unwrap();
                cmd_item = cmd_list.pop_front();
                cmd_list_clone = cmd_list.clone();
            }
            /*取出第一个命令后就可以put了。为了防止中途异常退出，丢失的命令越少越好，
            如果执行失败，酌情再插入list队尾
            逐条取出而不是一次性取出，也是为了防止状态改变，命令还没来得及插入的情况
            */
            match cmd_item {
                Some(item) => {
                    let cmd = item.cmd;
                    info!(
                        "will exec cmd, {}, new cmd list is: {}",
                        cmd.output(),
                        cmd_list_clone.output()
                    );

                    let _ = self.non_helper.put_cmd_list(&cmd_list_clone).await;

                    let app_id = cmd.app_id();
                    let cmd_code = cmd.cmd();

                    let status_list = self.status_list.read().unwrap().clone();
                    let status = status_list.get(app_id);
                    if status.is_none() {
                        let err_msg = format!(
                            "exec cmd [{:?}], but status not found in list! app:{}",
                            cmd.cmd(),
                            app_id
                        );
                        warn!("{}", err_msg);
                        continue;
                    }
                    let status = status.unwrap();

                    //以下执行，如果可以执行，但执行失败要返回Ok(${FailedState})，如果无法执行，比如前置状态不对，返回Err
                    match cmd_code {
                        CmdCode::Install(_) => {
                            let _ = self
                                .execute_install(status.clone(), &cmd, item.retry_count)
                                .await;
                        }
                        CmdCode::Uninstall => {
                            let _ = self
                                .execute_uninstall(status.clone(), &cmd, item.retry_count)
                                .await;
                        }
                        CmdCode::Start => {
                            let _ = self
                                .execute_start(status.clone(), &cmd, item.retry_count)
                                .await;
                        }
                        CmdCode::Stop => {
                            let _ = self
                                .execute_stop(status.clone(), &cmd, item.retry_count)
                                .await;
                        }
                        v @ _ => {
                            let err_msg = format!("cmd not executed!, app:{}, cmd:{:?}", app_id, v);
                            warn!("{}", err_msg);
                        }
                    };
                }
                None => break,
            }
        }
    }

    async fn execute_start(
        &self,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: &AppCmd,
        _retry_count: u32,
    ) -> BuckyResult<()> {
        let app_id = cmd.app_id();
        let cmd_code = cmd.cmd();
        //info!("will execute cmd, app:{}, cmd: {:?}", app_id, cmd_code);

        self.pre_change_status(
            app_id,
            status.clone(),
            cmd_code,
            AppLocalStatusCode::Starting,
            false,
        )
        .await?;

        let _ = self.app_controller.stop_app(app_id).await;

        //get quota
        let quota = status.lock().unwrap().quota().clone();
        let mut run_config = RunConfig {
            cpu_core: None,
            cpu_shares: if quota.cpu == 0 {
                None
            } else {
                Some(quota.cpu)
            },
            memory: if quota.mem == 0 {
                None
            } else {
                Some(quota.mem)
            },
            ip: None,
            network: None,
        };

        let mut sub_err = SubErrorCode::None;

        //获取App对应的容器IP
        loop {
            if self.use_docker {
                match self.docker_network_manager.get_valid_app_ip(app_id) {
                    Ok(ip) => {
                        info!("get ip for app:{}, ip: {}", app_id, ip);
                        if let Err(e) = self.register_app(app_id, &ip).await {
                            error!("register app to stack failed, app:{}, err:{}", app_id, e);
                            sub_err = SubErrorCode::RegisterAppFailed;
                            break;
                        }

                        run_config.ip = Some(ip);
                    }
                    Err(e) => {
                        error!("assign container ip failed, app:{}, err:{}", app_id, e);
                        sub_err = SubErrorCode::AssignContainerIpFailed;
                        break;
                    }
                }
                run_config.network = Some(CYFS_BRIDGE_NAME.to_owned());
            }

            if let Err(e) = self.app_controller.start_app(app_id, run_config).await {
                warn!("start app failed, app:{}, err:{}", app_id, e);
                sub_err = e;
            }

            break;
        }

        let mut target_status_code = AppLocalStatusCode::StartFailed;
        if sub_err == SubErrorCode::None {
            target_status_code = AppLocalStatusCode::Running;
            //let mut counters = self.start_couter.write().unwrap();
            //counters.insert(app_id.clone(), 0);
        }

        let _ = self
            .post_change_status(
                app_id,
                status.clone(),
                cmd_code,
                AppLocalStatusCode::Starting,
                target_status_code,
                sub_err,
            )
            .await;

        Ok(())
    }

    async fn execute_stop(
        &self,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: &AppCmd,
        _retry_count: u32,
    ) -> BuckyResult<()> {
        let app_id = cmd.app_id();
        let cmd_code = cmd.cmd();

        self.pre_change_status(
            app_id,
            status.clone(),
            cmd_code,
            AppLocalStatusCode::Stopping,
            false,
        )
        .await?;

        let mut target_status_code = AppLocalStatusCode::Stop;
        let mut sub_err = SubErrorCode::None;
        if let Err(e) = self.app_controller.stop_app(app_id).await {
            warn!("stop app failed, app:{}, err:{}", app_id, e);
            target_status_code = AppLocalStatusCode::StopFailed;
            sub_err = e;
        }

        let _ = self
            .post_change_status(
                app_id,
                status.clone(),
                cmd_code,
                AppLocalStatusCode::Stopping,
                target_status_code,
                sub_err,
            )
            .await;

        Ok(())
    }

    async fn execute_install(
        &self,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: &AppCmd,
        retry_count: u32,
    ) -> BuckyResult<()> {
        let app_id = cmd.app_id();
        let cmd_code = cmd.cmd();
        let ver;
        if let CmdCode::Install(param) = cmd_code {
            ver = param.ver.to_owned();
        } else {
            let err = format!("expect install cmd, cmd:{:?}", cmd_code);
            error!("{}", err);
            return Err(BuckyError::from((BuckyErrorCode::InvalidParam, err)));
        }
        status.lock().unwrap().set_version(&ver);

        self.pre_change_status(
            app_id,
            status.clone(),
            cmd_code,
            AppLocalStatusCode::Installing,
            true,
        )
        .await?;

        let mut sub_err = SubErrorCode::None;
        let target_status_code = match self
            .install_internal(status.clone(), cmd, retry_count, &ver)
            .await
        {
            Ok(v) => {
                //save app install detail to local file when install successfully
                let mut install_detail = AppInstallDetail::new(app_id);
                let _ = install_detail.set_install_version(Some(&ver));
                v
            }
            Err(e) => {
                sub_err = e;
                AppLocalStatusCode::InstallFailed
            }
        };

        let _ = self
            .post_change_status(
                app_id,
                status.clone(),
                cmd_code,
                AppLocalStatusCode::Installing,
                target_status_code,
                sub_err,
            )
            .await;

        Ok(())
        //ret.map(|_| ())
    }

    async fn install_internal(
        &self,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: &AppCmd,
        _retry_count: u32,
        version: &str,
    ) -> AppActionResult<AppLocalStatusCode> {
        let app_id = cmd.app_id();

        let dec_app = self
            .non_helper
            .get_dec_app(app_id.object_id(), None)
            .await
            .map_err(|e| {
                warn!("get dec app failed!, app:{}, err: {}", app_id, e);
                SubErrorCode::AppNotFound
            })?;

        // 获取权限配置并且设置到local status
        let permissions = self
            .app_controller
            .query_app_permission(app_id, version, &dec_app)
            .await
            .map_err(|e| {
                warn!("get app acl failed, app:{}, err: {}", app_id, e);
                SubErrorCode::QueryPermissionError
            })?;
        if let Some(permissions) = permissions {
            if status.lock().unwrap().add_permissions(&permissions) {
                //添加权限，如果权限有改变，put status.
                let status_clone = status.lock().unwrap().clone();
                let _ = self.non_helper.put_local_status(&status_clone).await;
            }
        }

        let (no_service, web_dir) = self
            .app_controller
            .install_app(app_id, version, &dec_app)
            .await?;

        let mut target_status_code = AppLocalStatusCode::Stop;
        if no_service {
            target_status_code = AppLocalStatusCode::NoService;
        }
        status.lock().unwrap().set_web_dir(web_dir);

        if let Some(obj_id) = web_dir {
            let _ = self
                .non_helper
                .save_app_web_dir(app_id, version, &obj_id)
                .await;
            let _ = self
                .non_helper
                .register_app_name(dec_app.name(), app_id)
                .await;
        }
        //版本已经提前设置了
        //status.lock().unwrap().set_version(version);

        Ok(target_status_code)
    }

    async fn execute_uninstall(
        &self,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd: &AppCmd,
        _retry_count: u32,
    ) -> BuckyResult<()> {
        let app_id = cmd.app_id();
        let cmd_code = cmd.cmd();

        self.pre_change_status(
            app_id,
            status.clone(),
            cmd_code,
            AppLocalStatusCode::Uninstalling,
            false,
        )
        .await?;

        //set install version to None when uninstall begin
        let mut install_detail = AppInstallDetail::new(app_id);
        let _ = install_detail.set_install_version(None);

        let web_id;
        let ver;
        {
            let status = status.lock().unwrap();
            web_id = status.web_dir().cloned();
            ver = status.version().unwrap().to_owned();
        }

        let mut target_status_code = AppLocalStatusCode::Uninstalled;
        let mut sub_err = SubErrorCode::None;
        if let Err(e) = self.app_controller.uninstall_app(app_id).await {
            //uninstall务必成功，这里先输出一个警告
            warn!("uninstall app failed, app:{}, err:{}", app_id, e);
            target_status_code = AppLocalStatusCode::UninstallFailed;
            sub_err = e;
        }
        if let Some(obj_id) = web_id {
            let _ = self
                .non_helper
                .remove_app_web_dir(app_id, &ver, &obj_id)
                .await;
            // if let Ok(dec_app) = self.non_helper.get_dec_app(app_id.object_id(), None).await {
            //     let _ = self
            //         .non_helper
            //         .unregister_app_name(dec_app.name(), app_id)
            //         .await;
            //     let _ = self
            //         .non_helper
            //         .remove_app_web_dir(app_id, &ver, &obj_id)
            //         .await;
            // }
        }

        let _ = self
            .post_change_status(
                app_id,
                status.clone(),
                cmd_code,
                AppLocalStatusCode::Uninstalling,
                target_status_code,
                sub_err,
            )
            .await;

        Ok(())
    }

    //注册app到协议栈
    async fn register_app(&self, app_id: &DecAppId, container_ip: &str) -> BuckyResult<()> {
        let app_name;
        match self.non_helper.get_dec_app(app_id.object_id(), None).await {
            Ok(dec_app) => {
                app_name = dec_app.name().to_owned();
            }
            Err(e) => {
                warn!("get dec app failed, err:{}", e);
                app_name = app_id.to_string();
            }
        }
        let dec_ip_info = DecIpInfo {
            name: app_name,
            ip: container_ip.to_owned(),
        };

        info!(
            "will register app to stack, app:{}, dec ip info:{:?}",
            app_id, dec_ip_info
        );

        let mut dec_map = HashMap::<String, DecIpInfo>::new();
        dec_map.insert(app_id.to_string(), dec_ip_info);
        let action = AppManagerAction::create_register_dec(
            self.owner.clone(),
            self.docker_network_manager.gateway_ip(),
            dec_map,
        );

        if let Err(e) = self.non_helper.post_object_without_resp(&action).await {
            error!("register app to stack failed. err:{}", e);
            return Err(e);
        }

        Ok(())
    }
    //执行命令之前，改前置状态，force_put:是否强制put一次状态
    async fn pre_change_status(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd_code: &CmdCode,
        pre_status: AppLocalStatusCode,
        force_put: bool,
    ) -> BuckyResult<()> {
        let status_clone;
        {
            let mut status = status.lock().unwrap();
            let cur_status_code = status.status();
            if cur_status_code == pre_status {
                //如果已经是需要的准备状态
                if !force_put {
                    return Ok(());
                }
            } else {
                if !Self::is_valid_pre_status(cmd_code, cur_status_code) {
                    let err_msg = format!(
                        "cannot execute cmd [{:?}], app:{}, status:{}, skip it",
                        cmd_code, app_id, cur_status_code
                    );
                    warn!("{}", err_msg);
                    return Err(BuckyError::from((BuckyErrorCode::ErrorState, err_msg)));
                }
                status.set_status(pre_status);
            }

            status_clone = status.clone();
        }

        let _ = self.non_helper.put_local_status(&status_clone).await;
        Ok(())
    }

    //转换app状态
    async fn post_change_status(
        &self,
        app_id: &DecAppId,
        status: Arc<Mutex<AppLocalStatus>>,
        cmd_code: &CmdCode,
        pre_status: AppLocalStatusCode,
        post_status: AppLocalStatusCode, //这是可能切换到的状态，因为可能会根据下一条命令提前切换到其他前置状态
        sub_error: SubErrorCode,
    ) -> BuckyResult<()> {
        let mut target_status_code = post_status;

        /*注意，这里需要判断一下后面的cmd，如果后面的cmd也是针对当前App的，说明可能是一系列复合命令，
        需要提前切换到下一状态的前置状态，否则可能会被新的命令改变成其他中间状态，比如，重启操作（先Stop后Start）
        需要先切换到Starting，如果切换到了Stop，那么新来一个命令是UnInstall，这里就会变成Uninstallling
        也许这里切换到Uninstalling后不执行Start，可能也是合理的，不过暂时先设计为命令串行执行，其他的方式后面再考虑*/
        //这里提前做，change_status_with_next_cmd里有cmdlist的锁，不和status的锁重合
        if let Some(code) = self.change_status_with_next_cmd(app_id, target_status_code) {
            target_status_code = code;
        }

        let status_clone;
        {
            let mut status = status.lock().unwrap();
            let cur_status_code = status.status();
            if cur_status_code != pre_status {
                //判断状态是否还是要求的前置状态，如果不是就不改变状态了
                let err_msg = format!(
                    "after execute cmd [{:?}], current status is [{}], app:{}, skip change status.",
                    cmd_code, cur_status_code, app_id
                );
                warn!("{}", err_msg);
                return Err(BuckyError::from((BuckyErrorCode::ErrorState, err_msg)));
            }

            info!(
                "after execute cmd [{:?}], change app from [{}] to [{}], app:{}",
                cmd_code, cur_status_code, target_status_code, app_id
            );
            status.set_sub_error(sub_error);
            status.set_status(target_status_code);
            status_clone = status.clone();
        }

        let _ = self.non_helper.put_local_status(&status_clone).await;
        Ok(())
    }

    // 判断当前status能不能执行cmd操作，比如Init状态可以Install，但是Stop状态不能Install
    pub fn is_valid_pre_status(cmd: &CmdCode, status: AppLocalStatusCode) -> bool {
        match cmd {
            CmdCode::Add(_) => true,
            CmdCode::Remove => {
                status == AppLocalStatusCode::Init
                    || status == AppLocalStatusCode::InstallFailed
                    || status == AppLocalStatusCode::UninstallFailed
                    || status == AppLocalStatusCode::Uninstalled
            }
            CmdCode::Install(_) => {
                status == AppLocalStatusCode::Init
                    || status == AppLocalStatusCode::InstallFailed
                    || status == AppLocalStatusCode::UninstallFailed
                    || status == AppLocalStatusCode::Uninstalled
            }
            CmdCode::Uninstall => {
                status == AppLocalStatusCode::Stop
                    || status == AppLocalStatusCode::StartFailed
                    || status == AppLocalStatusCode::NoService
                    || status == AppLocalStatusCode::RunException
                    || status == AppLocalStatusCode::UninstallFailed
                    || status == AppLocalStatusCode::StopFailed
                    || status == AppLocalStatusCode::Running
                    || status == AppLocalStatusCode::InstallFailed
            }
            CmdCode::Start => {
                status == AppLocalStatusCode::Stop
                    || status == AppLocalStatusCode::StartFailed
                    || status == AppLocalStatusCode::RunException
            }
            CmdCode::Stop => {
                status == AppLocalStatusCode::Running || status == AppLocalStatusCode::StopFailed
            }
            CmdCode::SetPermission(_) => {
                //安装的app才能设置权限，不然是浪费？
                status != AppLocalStatusCode::Init
            }
            CmdCode::SetQuota(_) => {
                //任何状态都可以设置配额？
                true
            }
            v @ _ => {
                warn!("[is_valid_pre_status], unknown cmd: {:?}", v);
                false
            }
        }
    }

    fn change_status_with_next_cmd(
        &self,
        app_id: &DecAppId,
        cur_status_code: AppLocalStatusCode,
    ) -> Option<AppLocalStatusCode> {
        let cmd_list = self.cmd_list.lock().unwrap();
        let list = cmd_list.list();
        for item in list {
            let cmd = &item.cmd;
            if cmd.app_id() == app_id {
                let cmd_code = item.cmd.cmd();
                if !Self::is_valid_pre_status(cmd_code, cur_status_code) {
                    continue;
                }
                let next_status = Self::get_next_status_with_cmd(cmd_code);
                if let Some(v) = next_status {
                    info!(
                        "next cmd is {:?}, will change app:{} to status: {}",
                        cmd_code, app_id, v
                    );
                }
                return next_status;
            }
        }
        None
        /*if let Some(item) = self.cmd_list.lock().unwrap().front() {
            if item.cmd.app_id() != app_id {
                return None;
            }
            let cmd_code = item.cmd.cmd();
            if !Self::is_valid_pre_status(cmd_code, cur_status_code) {
                return None;
            }
            let next_status = Self::get_next_status_with_cmd(cmd_code);
            if let Some(v) = next_status {
                info!(
                    "next cmd is {:?}, will change app:{} to status: {}",
                    cmd_code, app_id, v
                );
            }
            return next_status;
        }
        None*/
    }

    pub fn get_next_status_with_cmd(cmd: &CmdCode) -> Option<AppLocalStatusCode> {
        match cmd {
            CmdCode::Install(_) => Some(AppLocalStatusCode::Installing),
            CmdCode::Uninstall => Some(AppLocalStatusCode::Uninstalling),
            CmdCode::Start => Some(AppLocalStatusCode::Starting),
            CmdCode::Stop => Some(AppLocalStatusCode::Stopping),
            _ => {
                info!("status will not changed by cmd: {:?}", cmd);
                None
            }
        }
    }
}
