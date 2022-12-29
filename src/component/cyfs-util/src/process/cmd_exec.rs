use super::process_lock::ProcessLock;
use super::process_mutex::{ProcessMutex, CURRENT_PROC_LOCK, SERVICE_NAME};
use clap::{App, Arg, ArgMatches};
use std::error::Error;

pub trait ProcessCmdFuncs: Send + Sync {
    fn exit_process(&self, action: ProcessAction, code: i32) -> !;
}

struct DefaultProcessCmdFuncs;

impl ProcessCmdFuncs for DefaultProcessCmdFuncs {
    fn exit_process(&self, _action: ProcessAction, code: i32) -> ! {
        std::process::exit(code)
    }
}

static PROCESS_CMD_FUNCS: once_cell::sync::OnceCell<Box<dyn ProcessCmdFuncs>> =
    once_cell::sync::OnceCell::new();

pub fn get_process_cmd_funcs() -> &'static Box<dyn ProcessCmdFuncs> {
    PROCESS_CMD_FUNCS.get_or_init(|| Box::new(DefaultProcessCmdFuncs {}))
}

pub fn set_process_cmd_funcs(
    funcs: Box<dyn ProcessCmdFuncs>,
) -> Result<(), Box<dyn ProcessCmdFuncs>> {
    PROCESS_CMD_FUNCS.set(funcs)
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ProcessAction {
    Start,
    Stop,
    Status,
    Install,
}

#[derive(Debug)]
pub enum ProcessStatusCode {
    NotExists = 0,
    Running = 1,
    RunningOther = 2,
}

impl ProcessStatusCode {
    pub fn is_running_other(exit_code: i32) -> bool {
        if exit_code == Self::RunningOther as i32 {
            true
        } else {
            // compatible with old return values, which is -1 on windows and 255 on nix
            if exit_code == -1 || exit_code == 255 {
                true
            } else {
                false
            }
        }
    }
}
pub fn prepare_args<'a, 'b>(args: App<'a, 'b>) -> App<'a, 'b> {
    args.arg(
        Arg::with_name("start")
            .long("start")
            .takes_value(false)
            .help("Start service"),
    )
    .arg(
        Arg::with_name("stop")
            .long("stop")
            .takes_value(false)
            .help("Stop service that if service still running"),
    )
    .arg(
        Arg::with_name("status")
            .long("status")
            .takes_value(false)
            .help("Check service is running or not"),
    )
    .arg(
        Arg::with_name("install")
            .long("install")
            .takes_value(false)
            .help("Install service when first install or update"),
    )
    .arg(
        Arg::with_name("fid")
            .long("fid")
            .takes_value(true)
            .help("Current service package fid when check status"),
    )
    .arg(
        Arg::with_name("console-log")
            .long("console-log")
            .takes_value(false)
            .help("Use console log instead"),
    )
    .arg(
        Arg::with_name("cmd-log")
            .long("cmd-log")
            .takes_value(false)
            .help("Output full cmd log to {service-name}-{cmd} file"),
    )
}

pub fn parse_cmd(service_name: &str, matches: &ArgMatches) -> (ProcessAction, Option<String>) {
    let mut fid: Option<String> = None;
    let mut cmd = ProcessAction::Start;

    if matches.is_present("status") {
        cmd = ProcessAction::Status;

        match matches.value_of("fid") {
            Some(v) => fid = Some(v.to_string()),
            None => {
                warn!(
                    "fid param not specified with check status!!! service={}",
                    service_name
                );
            }
        }
    } else if matches.is_present("stop") {
        cmd = ProcessAction::Stop;
    } else if matches.is_present("install") {
        cmd = ProcessAction::Install;
    }

    (cmd, fid)
}

// 检测当前进程路径是不是匹配fid
fn check_current_fid(fid: &str) -> Result<bool, Box<dyn Error>> {
    let fid = fid.to_owned();

    let path = match std::env::current_exe() {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("get current_exe failed! err={}", e);
            error!("{}", msg);

            return Err(Box::<dyn Error>::from(msg));
        }
    };

    let path_str = match path.to_str() {
        Some(v) => v.to_owned(),
        None => {
            let msg = format!("get path str failed! path={}", path.display());
            error!("{}", msg);

            return Err(Box::<dyn Error>::from(msg));
        }
    };

    Ok(match path_str.find(&fid) {
        Some(_) => {
            debug!("fid found in exe path! fid={}, path={}", fid, path_str);
            true
        }
        None => {
            warn!("fid not found in exe path! fid={}, path={}", fid, path_str);
            false
        }
    })
}

static PROCESS_LOCK: once_cell::sync::OnceCell<ProcessLock> = once_cell::sync::OnceCell::new();

// 检查一个服务|APP的状态
pub fn check_process_status(service_name: &str, fid: Option<&str>) -> ProcessStatusCode {
    if ProcessMutex::new(service_name).acquire().is_some() {
        ProcessStatusCode::NotExists
    } else {
        let mut proc = ProcessLock::new(service_name);
        let exit_code = proc.check();
        if exit_code > 0 {
            info!(
                "target process in running! service={}, pid={}",
                service_name, exit_code
            );

            // 如果fid存在，那么检测是否是当前版本
            if fid.is_some() {
                match proc.check_fid(fid.unwrap()) {
                    Ok(valid) => {
                        if valid {
                            ProcessStatusCode::Running
                        } else {
                            ProcessStatusCode::RunningOther
                        }
                    }
                    Err(_e) => {
                        // 检测出错，那么都认为是running
                        ProcessStatusCode::Running
                    }
                }
            } else {
                ProcessStatusCode::Running
            }
        } else {
            info!(
                "target process mutex exists but plock not found! service={}, pid={}",
                service_name, exit_code
            );
            ProcessStatusCode::NotExists
        }
    }
}

pub fn check_cmd_and_exec(service_name: &str) -> ProcessAction {
    check_cmd_and_exec_ext(service_name, service_name)
}

pub fn check_cmd_and_exec_ext(service_name: &str, mutex_name: &str) -> ProcessAction {
    let about = format!("{} ood service for cyfs system", service_name);
    let app = App::new(&format!("{}", service_name))
        .version(cyfs_base::get_version())
        .about(&*about);

    let app = prepare_args(app);
    let matches = app.get_matches();

    check_cmd_and_exec_with_args_ext(service_name, mutex_name, &matches)
}

pub fn check_cmd_and_exec_with_args(service_name: &str, matches: &ArgMatches) -> ProcessAction {
    check_cmd_and_exec_with_args_ext(service_name, service_name, matches)
}

// 通过配置环境变量，让cmd输出日志
const CYFS_CMD_LOG_KEY: &str = "CYFS_CMD_LOG";

fn load_cmd_level_from_env() -> Option<String> {
    match std::env::var(CYFS_CMD_LOG_KEY) {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}

/*
stop
> 0 pid 成功停止了正在运行的service
0 service没有在运行
< 0 失败
*/
pub fn check_cmd_and_exec_with_args_ext(
    service_name: &str,
    mutex_name: &str,
    matches: &ArgMatches,
) -> ProcessAction {
    SERVICE_NAME.lock().unwrap().init(mutex_name);

    let (cmd, fid) = parse_cmd(service_name, matches);

    // 如果cmd=start，那么直接使用应用层自己的初始化日志逻辑
    if cmd != ProcessAction::Start {
        // 环境变量和命令行都可以控制cmd日志的开启
        let cmd_log_level = load_cmd_level_from_env();

        if cmd_log_level.is_some() || matches.is_present("cmd-log") {
            let name = format!("{}-status", service_name);
            let level = cmd_log_level.unwrap_or("trace".to_owned());

            crate::init_log(&name, Some(&level));
        } else {
            let console_log = matches.is_present("console-log");
            if console_log || (cmd != ProcessAction::Start && cmd != ProcessAction::Install) {
                if let Err(e) = simple_logger::SimpleLogger::default().init() {
                    println!("init simple logger error! {}", e);
                }
            } else {
                // start和install模式交给service本身决定，一般使用文件日志
            }
        }
    }
    // 如果不是启动，那么需要确保几秒后退出
    if cmd != ProcessAction::Start {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            let code = ProcessStatusCode::NotExists as i32;
            error!("process running out of time! now will exit with {}", code);
            crate::flush_log();
            get_process_cmd_funcs().exit_process(cmd, code);
        });
    }

    match cmd {
        ProcessAction::Stop => {
            if *CURRENT_PROC_LOCK {
                info!("process mutex not exists! service={}", service_name);
                get_process_cmd_funcs().exit_process(cmd, 0);
            }

            let exit_code = try_stop_process(service_name);
            get_process_cmd_funcs().exit_process(cmd, exit_code);
        }
        ProcessAction::Status => {
            // 如果进程锁不存在，那么该进程一定不存在
            if *CURRENT_PROC_LOCK {
                info!("process mutex not exists! service={}", service_name);
                get_process_cmd_funcs().exit_process(cmd, ProcessStatusCode::NotExists as i32);
            }

            let mut proc = ProcessLock::new(service_name);
            let exit_code = proc.check();
            let status;
            if exit_code > 0 {
                info!(
                    "target process in running! service={}, pid={}",
                    service_name, exit_code
                );

                if fid.is_some() {
                    status = match proc.check_fid(&fid.unwrap()) {
                        Ok(valid) => {
                            if valid {
                                ProcessStatusCode::Running
                            } else {
                                ProcessStatusCode::RunningOther
                            }
                        }
                        Err(_e) => {
                            // 检测出错，那么都认为是running
                            ProcessStatusCode::Running
                        }
                    };
                } else {
                    status = ProcessStatusCode::Running;
                }
            } else {
                info!(
                    "target process not found! service={}, pid={}",
                    service_name, exit_code
                );
                status = ProcessStatusCode::NotExists;
            }

            info!(
                "check status return, service={}, status={:?}",
                service_name, status
            );
            get_process_cmd_funcs().exit_process(cmd, status as i32);
        }
        ProcessAction::Install => {
            return ProcessAction::Install;
        }
        _ => {
            if !*CURRENT_PROC_LOCK {
                let msg = format!("process mutex already exists! service={}", service_name);
                warn!("{}", msg);
                println!("{}", msg);

                get_process_cmd_funcs().exit_process(cmd, 1);
            }

            let mut proc = ProcessLock::new(service_name);
            if let Err(e) = proc.force_acquire() {
                let msg = format!(
                    "target process already in running! service={}, pid={}, err={:?}",
                    service_name,
                    proc.get_old_pid(),
                    e
                );
                error!("{}", msg);
                println!("{}", msg);

                get_process_cmd_funcs().exit_process(cmd, 1);
            }

            if let Err(_) = PROCESS_LOCK.set(proc) {
                unreachable!();
            }

            return ProcessAction::Start;
        }
    }
}

// 只通过进程锁检查一个进程是否存在
pub fn check_process_mutex(service_name: &str) -> bool {
    let mutex = ProcessMutex::new(&service_name);
    let ret = match mutex.acquire() {
        Some(g) => {
            drop(g);
            false
        }
        None => true,
    };

    ret
}

// 不检查进程锁，直接读取进程pid文件并尝试终止
pub fn try_stop_process(service_name: &str) -> i32 {
    let mut proc = ProcessLock::new(service_name);
    let pid = proc.check();
    if pid > 0 {
        info!(
            "target process in running! now will kill, service={}, pid={}",
            service_name, pid
        );

        let exit_code = match proc.kill() {
            true => pid as i32,
            false => -1i32,
        };

        exit_code
    } else {
        info!(
            "process in running but pid file not exists! {}",
            service_name
        );
        -1
    }
}

// 尝试获取指定的进程锁，如果成功则创建对应的pid文件
pub fn try_enter_proc(service_name: &str) -> bool {
    SERVICE_NAME.lock().unwrap().init(service_name);

    if !*CURRENT_PROC_LOCK {
        info!("process mutex already exists! service={}", service_name);
        return false;
    }

    let mut proc = ProcessLock::new(service_name);
    if let Err(e) = proc.force_acquire() {
        error!(
            "target process already in running! service={}, pid={}, err={:?}",
            service_name,
            proc.get_old_pid(),
            e
        );

        return false;
    }

    if let Err(_) = PROCESS_LOCK.set(proc) {
        unreachable!();
    }

    true
}
