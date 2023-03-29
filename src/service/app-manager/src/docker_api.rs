use cyfs_base::*;
use cyfs_util::*;
use log::*;
use std::ffi::OsStr;
use std::fmt::format;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::Duration;
use crate::docker_network_manager::*;
use itertools::Itertools;
use wait_timeout::ChildExt;
use crate::dapp::INSTALL_CMD_TIME_OUT_IN_SECS;

#[derive(Debug)]
pub struct RunConfig {
    // 绝对限制 1 ==> 1C
    pub cpu_core: Option<f64>,
    // cpu 相对限制
    pub cpu_shares: Option<i64>,
    // 单位 MiB:  ->  *1048576   = xxx bytes
    pub memory: Option<i64>,

    pub network: Option<String>,
    pub ip: Option<String>,
}

impl Default for RunConfig {
    fn default() -> RunConfig {
        RunConfig {
            cpu_core: None,
            cpu_shares: Some(100),
            memory: Some(1024),
            network: None,
            ip: None,
        }
    }
}

const APP_BASE_IMAGE: &str = "buckyos/dec-app-base";
const APP_BASE_TAG: &str = "v1";

// entry strip content
// 有些配置需要在docker run之后处理
// start script 注入docker entrypoint, app run的命令设置在docker run cmd(作为首个参数)
// iptables: alternative set the corret iptables version which defined by host kenel mod
const START_SHELL: &'static str = r#"
#!/bin/bash
container_ip=`hostname -i`
result=$(iptables -L 2>&1)
if [[ "$result" == *"iptables-legacy tables present"* ]]; then
    echo 'kenel mod tables is not nftables. switch client to iptables-legacy'
    update-alternatives --set iptables /usr/sbin/iptables-legacy
fi
iptables -t nat -F

iptables -t nat -A OUTPUT -d 127.0.0.1/32 -p tcp -m tcp --dport 1318 -j DNAT --to-destination {gateway_ip}:1318
iptables -t nat -A OUTPUT -d 127.0.0.1/32 -p tcp -m tcp --dport 1319 -j DNAT --to-destination {gateway_ip}:1319
iptables -t nat -A POSTROUTING -s 127.0.0.1 -p tcp -j SNAT --to-source $container_ip
# iptables-save

# sysctl -w net.ipv4.conf.eth0.route_localnet=1

# now the primary process back into the foreground
$1"#;

/// 基础镜像
/// cyfs-base 镜像处理好前置需要网络安装和更新 Dockfile:
/// FROM debian
/// RUN apt update -y && apt install -y iptables net-tools telnet curl procps
/// RUN curl -sL https://deb.nodesource.com/setup_16.x | bash -
/// RUN apt update -y && apt install -y nodejs
/// WORKDIR /opt/app
/// ENTRYPOINT [\"bash\", \"/opt/start.sh\"]

fn run_docker<S: AsRef<OsStr>>(args: Vec<S>) -> BuckyResult<Child> {
    let mut cmd = Command::new("docker");
    cmd.args(args);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let args_str = cmd.get_args().collect::<Vec<&OsStr>>().iter().map(|s|s.to_string_lossy()).join(" ");
    info!("will spawn cmd: docker {}", &args_str);

    return Ok(cmd.spawn().map_err(|e| {
        error!("spawn cmd: docker {} err {}", args_str, e);
        e
    })?);
}

fn run_docker_only_status<S: AsRef<OsStr>>(args: Vec<S>) -> BuckyResult<ExitStatus> {
    Ok(run_docker(args)?.wait()?)
}

fn add_bind_volume<P: AsRef<Path>, Q: AsRef<Path>>(args: &mut Vec<String>, source: P, target: Q, read_only: bool) {
    args.push("-v".to_owned());
    let mounts = if read_only {
        format!("{}:{}:ro", source.as_ref().to_string_lossy().to_string(), target.as_ref().to_string_lossy().to_string())
    } else {
        format!("{}:{}", source.as_ref().to_string_lossy().to_string(), target.as_ref().to_string_lossy().to_string())
    };
    args.push(mounts)
}

fn code_to_string(code: Option<i32>) -> String {
    code.map(|i|{i.to_string()}).unwrap_or("signal".to_owned())
}

pub struct DockerApi {
    host_reslov: PathBuf
}

impl DockerApi {
    pub fn new() -> Self {
        // dns
        // if host machine's resolv.conf contain the '127.0.0.53', it means host use systemd resolv service, which container can not use.
        // In this time , we should not mont the /etc/resolv to container
        // docker has some handler, can see this source:
        // https://github.com/docker/docker-ce/blob/44a430f4c43e61c95d4e9e9fd6a0573fa113a119/components/engine/libnetwork/resolvconf/resolvconf.go#L52
        // https://superuser.com/questions/1702091/how-should-systemd-resolved-and-docker-interact
        let mut host_reslov = PathBuf::from("/etc/resolv.conf");
        if let Ok(resolv_content) = std::fs::read_to_string(&host_reslov) {
            let is_host_systemd_resolved = resolv_content.contains("127.0.0.53");
            if is_host_systemd_resolved {
                info!("resolv.conf file contain 127.0.0.53, use systemd resolv.conf instead");
                host_reslov = PathBuf::from("/run/systemd/resolve/resolv.conf");
            } else {
                info!("resolv.conf file did not contain 127.0.0.53, use this file directly");
            }
        } else {
            warn!("read resolv file err");
            host_reslov = PathBuf::new();
        }
        Self { host_reslov }
    }

    fn get_hostconfig_mounts(&self, id: &str) -> Vec<String> {
        info!("start to handle container mount params of app:{}", id);
        let mut mounts = vec![];

        // mount log 目录
        let log_dir = get_app_log_dir(id);
        let default_log_dir = default_cyfs_root_path().join("log").join("app");

        let app_data_dir = get_app_data_dir(id);
        let default_app_data_dir = get_app_data_dir_ex(id, &default_cyfs_root_path());

        add_bind_volume(&mut mounts, log_dir, default_log_dir, false);
        add_bind_volume(&mut mounts, app_data_dir, default_app_data_dir, false);

        // 将启动脚本mount到/opt/start.sh
        add_bind_volume(&mut mounts, get_app_dockerfile_dir(id).join("start.sh"), "/opt/start.sh", true);

        let tmp_path = get_temp_path();
        let default_tmp_path = default_cyfs_root_path().join("tmp");

        add_bind_volume(&mut mounts, tmp_path, default_tmp_path, false);

        add_bind_volume(&mut mounts, "/etc/localtime", "/etc/localtime", true);

        if self.host_reslov.is_file() {
            info!("use host reslov file {} as /etc/resolv.conf", self.host_reslov.display());
            add_bind_volume(&mut mounts, &self.host_reslov, "/etc/resolv.conf", true);
        }

        mounts
    }

    pub async fn update_image() -> BuckyResult<()> {
        let output = String::from_utf8(run_docker(vec![
            "images",
            "--format", "{{.Tag}}",
            APP_BASE_IMAGE])?.wait_with_output()?.stdout).unwrap();

        for line in output.lines() {
            if line == APP_BASE_TAG {
                info!("app image tag {} exists", line);
                return Ok(());
            }
        }

        info!("cannot found app image tag {}, pull it", APP_BASE_TAG);

        let ret = run_docker_only_status(vec!["pull".to_string(), format!("{}:{}", APP_BASE_IMAGE, APP_BASE_TAG)])?;
        if !ret.success() {
            return Err(BuckyError::new(BuckyErrorCode::Failed, format!("docker pull image failed")));
        };

        Ok(())
    }

    pub fn get_network_gateway_ip() -> BuckyResult<String> {
        // docker network inspect --format='{{range .IPAM.Config}}{{.Gateway}}{{end}}' cyfs_br
        let output = run_docker(vec!["network",
                                     "inspect",
                                     "--format",
                                     "{{range .IPAM.Config}}{{.Gateway}}{{end}}",
                                     CYFS_BRIDGE_NAME])?.wait_with_output().map_err(|e| {
            error!("get_network_gateway_ip cmd error {:?}", e);
            e
        })?;

        if output.status.success() {
            let ip = String::from_utf8(output.stdout).unwrap().trim().to_string();
            info!("get network Gateway ip {}", ip);
            Ok(ip)
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::Failed,
                "docker ls network get Gateway ip failed",
            ))
        }
    }

    fn prepare(&self, id: &str) -> BuckyResult<()> {
        // dockerfile  目录
        let dockerfile_dir = get_app_dockerfile_dir(&id);
        if !dockerfile_dir.exists() {
            std::fs::create_dir_all(dockerfile_dir.clone())?;
        }

        // 构造 start.sh
        let start_sh_path = dockerfile_dir.join("start.sh");
        let gateway_ip = DockerApi::get_network_gateway_ip()?;
        info!("get docker start shell 's gateway ip: {}", gateway_ip);

        // network的 gateway_ip是manager启动后生成的。这里需要根据名字（名字是固定的）需要获取后替换进去启动脚本里
        let shell = START_SHELL
            .replace("{gateway_ip}", &gateway_ip);
        info!("docker start shell content: {}", &shell);
        std::fs::write(&start_sh_path, shell)?;
        Ok(())
    }

    pub async fn install(
        &self,
        id: &str,
        _version: &str,
        install_cmd: Vec<String>,
    ) -> BuckyResult<()> {
        self.prepare(id)?;
        // TODO 在容器内执行install命令
        for cmd in install_cmd {
            info!("start app {} install cmd {} in docker", id, cmd);
            let container_name = format!("decapp-{}-install", id.to_lowercase());
            let mut create_args = vec![
                "run".to_string(),
                "--name".to_string(), container_name.clone(),
                "--cap-add".to_string(), "NET_ADMIN".to_string(),
                "--cap-add".to_string(), "NET_RAW".to_string(),
                "--rm".to_string(), "--init".to_string(),
                "--network".to_string(), "bridge".to_string(),
            ];

            // 容器启动的host配置
            let mut mounts = self.get_hostconfig_mounts(id);
            // 将app service目录mount到/opt/app, app install的时候service目录为可写
            add_bind_volume(&mut mounts, get_app_dir(id), "/opt/app", false);
            create_args.append(&mut mounts);

            create_args.push(format!("{}:{}", APP_BASE_IMAGE, APP_BASE_TAG));
            create_args.push(self.fix_cmd(id, cmd.clone()));

            let mut child = run_docker(create_args).map_err(|e| {
                error!("app {} run install cmd {} err {}", id, &cmd, e);
                e
            })?;

            match child.wait_timeout(Duration::from_secs(INSTALL_CMD_TIME_OUT_IN_SECS))? {
                None => {
                    error!("app {} run install cmd {} not return after {} secs, kill", id, &cmd, INSTALL_CMD_TIME_OUT_IN_SECS);
                    child.kill();
                    child.wait();
                }
                Some(status) => {
                    if status.success() {
                        info!("app {} run install cmd {} success", id, &cmd);
                    } else {
                        error!("app {} run install cmd {}, exit code {}", id, &cmd, code_to_string(status.code()));
                        return Err(BuckyError::from(BuckyErrorCode::Failed));
                    }
                }
            }
        }
        Ok(())
    }

    /// uninstall
    /// remove image, remove dockerfile.gz
    pub async fn uninstall(&self, id: &str) -> BuckyResult<()> {
        let _ = self._remove_dockerfile(id);

        Ok(())
    }

    fn _remove_dockerfile(&self, id: &str) -> BuckyResult<()> {
        let dockerfile_dir = get_app_dockerfile_dir(&id);
        if dockerfile_dir.exists() {
            std::fs::remove_dir_all(dockerfile_dir.clone())?;
        }
        Ok(())
    }

    fn fix_cmd(&self, id: &str, cmd: String) -> String {
        let mut args: Vec<&str> = ProcessUtil::parse_cmd(&cmd);
        if args.len() == 0 {
            return cmd;
        }

        let may_path = get_app_dir(id).join(args[0]);
        if may_path.exists() {
            let new_program = format!("/opt/app/{}", args[0]);
            args[0] = &new_program;

            let new_cmd = args.iter().map(|s| {
                if s.contains(" ") {
                    format!("\"{}\"", s)
                } else {
                    s.to_string()
                }
            }).join(" ");
            info!("fix cmd \"{}\" to \"{}\"", &cmd, &new_cmd);
            return new_cmd;
        }

        cmd
    }

    // 运行容器
    pub async fn start(
        &self,
        id: &str,
        config: RunConfig,
        command: String,
    ) -> BuckyResult<()> {
        info!("docker run dec app:{}, config {:?}", id, config);
        if self.is_running(id)? {
            info!("docker container is alreay running {}", id);
            return Ok(());
        }
        let container_name = format!("decapp-{}", id.to_lowercase());
        // 每次启动前都prepare，对应start脚本修改的情况
        self.prepare(id)?;
        // 兼容：移除旧版本停止但没有删除的container
        let _ = run_docker(vec!["rm", "-f", &container_name])?.wait();
        // build options
        // docker run --name ${container_name} --network none ${mounts} --cap-add NET_ADMIN --cap-add NET_RAW --sysctl net.ipv4.conf.eth0.route_localnet=1 --log-driver json-file --log-opt max-size=100m --log-opt max-file=3

        let mut create_args = vec![
            "run".to_string(),
            "--name".to_string(), container_name.clone(),
            "--cap-add".to_string(), "NET_ADMIN".to_string(),
            "--cap-add".to_string(), "NET_RAW".to_string(),
            "--sysctl".to_string(), "net.ipv4.conf.eth0.route_localnet=1".to_string(),
            "--log-driver".to_string(), "json-file".to_string(),
            "--log-opt".to_string(), "max-size=100m".to_string(),
            "--log-opt".to_string(), "max-file=3".to_string(),
            "--rm".to_string(), "-d".to_string(), "--init".to_string()
        ];

        // 容器启动的host配置
        let mut mounts = self.get_hostconfig_mounts(id);
        // 将app service目录mount到/opt/app, app run的时候service目录为只读
        add_bind_volume(&mut mounts, get_app_dir(id), "/opt/app", true);
        create_args.append(&mut mounts);

        // 内存限制
        if let Some(memory) = config.memory {
            create_args.push("--memory".to_string());
            create_args.push((memory * 1048576).to_string());
        }

        // cpu绝对限制
        if let Some(cpu_core) = config.cpu_core {
            let cpu_quota = (cpu_core * f64::from(100000)).round() as i64;
            create_args.push("--cpu-period".to_string());
            create_args.push("100000".to_string());

            create_args.push("--cpu-quota".to_string());
            create_args.push(cpu_quota.to_string());
        }

        // cpu相对限制
        if let Some(cpu_shares) = config.cpu_shares {
            create_args.push("--cpu-shares".to_string());
            create_args.push(cpu_shares.to_string());
        }

        // ip和network 配置
        // 通过docker network inspect cyfs_br 可以快速查看container的ip是否配置正确
        if config.ip.is_some() && config.network.is_some() {
            create_args.push("--network".to_string());
            create_args.push(config.network.unwrap());

            create_args.push("--ip".to_string());
            create_args.push(config.ip.unwrap());
        } else {
            create_args.push("--network".to_string());
            create_args.push("none".to_string());
        }

        create_args.push(format!("{}:{}", APP_BASE_IMAGE, APP_BASE_TAG));
        create_args.push(self.fix_cmd(id, command));

        info!("dapp start: run container, {}", id);
        let output = run_docker_only_status(create_args).map_err(|e| {
            error!("run container {} err {}", container_name, e);
            e
        })?;

        if output.success() {
            info!("run container {} success", container_name);
            Ok(())
        } else {
            error!("run container {} fail, exit code {}", container_name, output.code().map(|i|{i.to_string()}).unwrap_or("signal".to_owned()));
            Err(BuckyError::from(BuckyErrorCode::Failed))
        }
    }

    pub fn stop(&self, id: &str) -> BuckyResult<()> {
        let container_name = format!("decapp-{}", id.to_lowercase());
        info!("try to stop container[{}]", container_name);
        if !(self.is_running(id)?) {
            info!("container {} not running", container_name);
            return Ok(());
        }

        let args = vec!["stop", "-t", "30", &container_name];

        let output = run_docker_only_status(args).map_err(|e| {
            error!("stop container {} err {}", container_name, e);
            e
        })?;

        if output.success() {
            info!("stop container {} success", container_name);
            Ok(())
        } else {
            error!("stop container {} fail, exit code {}", container_name, output.code().map(|i|{i.to_string()}).unwrap_or("signal".to_owned()));
            Err(BuckyError::from(BuckyErrorCode::Failed))
        }
    }

    pub fn is_running(&self, id: &str) -> BuckyResult<bool> {
        let container_name = format!("decapp-{}", id.to_lowercase());
        let output = run_docker(vec!["container", "inspect", &container_name, "--format", "{{.State.Status}}"])?.wait_with_output()?;
        if output.status.success() {
            let status = String::from_utf8(output.stdout).unwrap();
            Ok(status.trim() == "running")
        } else {
            info!("container inspect return error, tract as not running");
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    const id: &'static str = "9tgplnna8uvtpycfv1lbrn2bqa5g9vrbkhdhziwjd7wa";
    const version: &'static str = "1.2.2";

    #[async_std::test]
    async fn test_docker_clean_all() {
        test_docker_container_stop();
        test_docker_image_remove();
    }

    #[async_std::test]
    async fn test_docker_buildup() {
        test_docker_image_build_prepare();
        test_docker_image_build();
        test_docker_container_run();
    }

    /// prepare
    #[async_std::test]
    async fn test_docker_image_build_prepare() {
        // remove dockerfile.gz
        let dockerfile_gz_path = get_dockerfile_gz_path(&id, &version).unwrap();
        info!("dockerfile_gz_path {:?}", dockerfile_gz_path);
        std::fs::remove_file(dockerfile_gz_path);
        info!("delete dockerfile.gz file");

        prepare(id, version, None);
        info!("prepare complete");
    }

    // 运行 容器
    #[async_std::test]
    async fn test_docker_container_run() {
        cyfs_debug::CyfsLoggerBuilder::new_service(APP_MANAGER_NAME)
            .level("debug")
            .console("debug")
            .enable_bdt(Some("debug"), Some("debug"))
            .build()
            .unwrap()
            .start();

        let docker_api = DockerApi::new();
        // 0.5C 0.5G
        let config = RunConfig {
            ..Default::default()
        };
        let resp = docker_api
            .start(
                id,
                config,
                //Some(vec!["tail".to_string(), "-f".to_string(), "/dev/null".to_string(),])
                "tail -f /dev/null".to_string(),
            )
            .await;

        info!("resp {:?}", resp);

        let output = Command::new("docker").args(["ps"]).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        info!("stdout: docker ps  {}", stdout);
    }

    // 运行 容器 with 设置ip
    #[async_std::test]
    async fn test_docker_container_run_with_ip() {
        let docker_api = DockerApi::new();
        // 0.5C 0.5G
        let config = RunConfig {
            network: Some("my-bridge-network".to_string()),
            ip: Some("172.17.0.100".to_string()),
            ..Default::default()
        };
        let resp = docker_api
            .start(
                id,
                config,
                //Some(vec!["tail".to_string(), "-f".to_string(), "/dev/null".to_string(),])
                "tail -f /dev/null".to_string(),
            )
            .await;

        info!("resp {:?}", resp);
    }

    #[async_std::test]
    async fn test_docker_container_stop() {
        let name = format!("decapp-{}:{}", id.to_lowercase(), version);

        let docker_api = DockerApi::new();
        let resp = docker_api.stop(id).await;
        info!("remove result {:?}", resp);

        let exist = docker_api.is_running(&name).unwrap();

        assert!(!exist);
    }

    #[async_std::test]
    async fn test_docker_container_is_running() {
        let docker_api = DockerApi::new();
        let is_running = docker_api.is_running(id).await.unwrap();

        info!("container is running: {:?}", is_running);
    }

    #[async_std::test]
    async fn test_docker_get_image() {
        let docker_api = DockerApi::new();
        let resp = docker_api.get_image(id).await.unwrap();
        info!("docker get image with tag {:?}", resp);
    }
}
