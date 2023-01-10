use bollard::Docker;
use cyfs_base::*;
use cyfs_util::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::stream::StreamExt;
use log::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use tar::Header;
// use bollard::container::*;
use crate::docker_network_manager::*;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::image::{BuildImageOptions, ListImagesOptions};
use bollard::models::*;
use bollard::volume::{CreateVolumeOptions, RemoveVolumeOptions};

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

// entry strip content
// 有些配置需要在docker run之后处理
// start script 注入docker entrypoint, app run的命令设置在docker run cmd(作为首个参数)
// iptables: alternative set the corret iptables version which defined by host kenel mod
const START_SHELL: &'static str = r#"
#!/bin/bash
set -m

# workdir 
cd /opt/app

{executable}

{executable_relative_path}$1 &

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
fg %1"#;

/// 基础镜像
/// cyfs-base 镜像处理好前置需要网络安装和更新 Dockfile:
/// FROM debian
/// RUN apt update -y && apt install -y iptables net-tools telnet curl procps
/// RUN curl -sL https://deb.nodesource.com/setup_16.x | bash -
/// RUN apt update -y && apt install -y nodejs
const BASE_DOCKER_FILE: &'static str = "
FROM alexsunxl/cyfs-base
ADD app /opt/app
ADD start.sh /opt/start.sh
ENTRYPOINT [\"bash\", \"/opt/start.sh\"]";

/// ---
///
/// # Build Image的准备工作
/// prepare 为docker build 提供一些前置的文件处理
/// 通过unix socket 发送build 请求， 请求body里需要带 tar.gz
/// tar.gz的内容主要是Dockerfile 以及需要Dockerfile ADD指令所标记的文件
/// （在这里主要是dec app的代码文件）
/// 最终的指令大致是:
///     curl -v --unix-socket /var/run/docker
///         -X POST -H "Content-Type:application/tar"
///         --data-binary '@Dockerfile.tar.gz'  "http://localhost/v1.41/build?t=image_name"
///
/// # Arguments
/// - DecAppId decapp的id
pub fn prepare(id: &str, version: &str, executable: Option<Vec<String>>) -> BuckyResult<()> {
    info!("prepare docker build start {} {}", id, version);

    // dockerfile  目录
    let dockerfile_dir = get_app_dockerfile_dir(&id);
    if !dockerfile_dir.exists() {
        std::fs::create_dir_all(dockerfile_dir.clone())?;
    }

    let dockerfile_gz_path = get_dockerfile_gz_path(&id, &version)?;
    if dockerfile_gz_path.exists() {
        let _ = std::fs::remove_file(dockerfile_gz_path.clone());
    }

    // 处理dockerfile, 为 ADD 应用目录做准备
    let dockerfile_gz = File::create(dockerfile_gz_path)?;
    let enc = GzEncoder::new(dockerfile_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    info!(
        "prepare docker build add app dir -> Dockerfile.tar  {} {}",
        id, version
    );

    // 添加 app 目录
    tar.append_dir_all("app", get_app_dir(&id))?;

    // 添加 start.sh
    {
        let gateway_ip = DockerApi::get_network_gateway_ip()?;
        info!("get docker start shell 's gateway ip: {}", gateway_ip);

        // 用可执行命令运行的app，需要在容器内执行一下 chmod +x (e.g.  app.service.type:rust)
        let (chmod_executable, executable_relative_path) = {
            if executable.is_some() {
                let executable = executable.unwrap();
                let result = executable
                    .into_iter()
                    .map(|exec| format!("chmod +x {}", exec))
                    .collect::<Vec<String>>()
                    .join(" && ");
                (result, "./".to_string())
            } else {
                ("".to_string(), "".to_string())
            }
        };

        info!(
            "get docker start shell 's chmod executable: {}",
            chmod_executable
        );

        // network的 gateway_ip是manager启动后生成的。这里需要根据名字（名字是固定的）需要获取后替换进去启动脚本里
        let shell = START_SHELL
            .replace("{executable}", &chmod_executable)
            .replace("{executable_relative_path}", &executable_relative_path)
            .replace("{gateway_ip}", &gateway_ip);
        info!("docker start shell content: {}", shell);
        let data = shell.as_bytes();

        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_cksum();
        let _ = tar.append_data(&mut header, "start.sh", data);
    }

    // 添加 Dockerfile
    {
        let data = BASE_DOCKER_FILE.as_bytes();
        let mut header = Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, "Dockerfile", data)?;
    }

    info!("prepare docker build finish  {} {}", id, version);
    Ok(())
}

fn get_dockerfile_gz_path(id: &str, version: &str) -> BuckyResult<PathBuf> {
    let target = get_app_dockerfile_dir(&id).join(format!("Dockerfile_{}.tar.gz", version));
    Ok(target)
}

fn get_hostconfig_mounts(id: &str) -> BuckyResult<Option<Vec<Mount>>> {
    info!("start to handle container mount params of app:{}", id);

    // mount log 目录
    let log_dir = get_cyfs_root_path().join("log").join("app").join(id);
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir.clone())?;
    }

    let app_data_dir = get_cyfs_root_path().join("data").join("app").join(id);
    if !app_data_dir.exists() {
        std::fs::create_dir_all(app_data_dir.clone())?;
    }

    let mut mounts = vec![
        Mount {
            target: Some("/cyfs/log/app".to_string()),
            source: Some(log_dir.as_path().display().to_string()),
            typ: Some(bollard::models::MountTypeEnum::BIND),
            read_only: Some(false),
            ..Default::default()
        },
        Mount {
            target: Some(format!("/cyfs/data/app/{}", id)),
            source: Some(app_data_dir.as_path().display().to_string()),
            typ: Some(bollard::models::MountTypeEnum::BIND),
            read_only: Some(false),
            ..Default::default()
        },
        Mount {
            target: Some("/cyfs/tmp".to_string()),
            source: Some(get_temp_path().into_os_string().into_string().unwrap()),
            typ: Some(bollard::models::MountTypeEnum::BIND),
            read_only: Some(false),
            ..Default::default()
        },
        Mount {
            // bind /etc/localtime 让容器内和宿主机的时区保持一致
            target: Some("/etc/localtime".to_string()),
            source: Some("/etc/localtime".to_string()),
            typ: Some(bollard::models::MountTypeEnum::BIND),
            read_only: Some(true),
            ..Default::default()
        },
    ];

    // dns
    // if host machine's resolv.conf contain the '127.0.0.53', it means host use systemd resolv service, which container can not use.
    // In this time , we should not mont the /etc/resolv to container
    // docker has some handler, can see this source:
    // https://github.com/docker/docker-ce/blob/44a430f4c43e61c95d4e9e9fd6a0573fa113a119/components/engine/libnetwork/resolvconf/resolvconf.go#L52
    // https://superuser.com/questions/1702091/how-should-systemd-resolved-and-docker-interact
    let resolv = "/etc/resolv.conf";
    let resolv_content = std::fs::read_to_string(resolv).map_err(|err| {
        BuckyError::new(
            BuckyErrorCode::Failed,
            format!("failed to read the resolv file: {}", err),
        )
    })?;
    let reg = regex::Regex::new(r"127.0.0.53").unwrap();
    let is_host_systemd_resolved = resolv_content.contains("127.0.0.53");
    if !is_host_systemd_resolved {
        mounts.push(Mount {
            // container's dns  conf bind host config
            target: Some("/etc/resolv.conf".to_string()),
            source: Some("/etc/resolv.conf".to_string()),
            typ: Some(bollard::models::MountTypeEnum::BIND),
            read_only: Some(true),
            ..Default::default()
        })
    }

    Ok(Some(mounts))
}

pub struct DockerApi {
    docker: Docker,
}

impl DockerApi {
    pub fn new() -> DockerApi {
        let docker = Docker::connect_with_socket_defaults().unwrap();
        DockerApi { docker: docker }
    }

    /// # get_network_id
    /// 获取network id. container 的启动config里，除了network的名字还需要id
    pub fn get_network_id(network_name: &str) -> BuckyResult<String> {
        let name_filter = format!("name={}", network_name);
        info!(
            "get_network_id cmd: docker network network ls --filter {:?}  --no-trunc",
            name_filter
        );
        let output = Command::new("docker")
            .args(["network", "ls", "--filter", &name_filter, "--no-trunc"])
            .output()
            .map_err(|e| {
                error!("get_network_id cmd error {:?}", e);
                BuckyError::new(BuckyErrorCode::Failed, "get_network_id cmd error")
            })?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).unwrap();
            // 第二行的第一个
            let result = stdout.lines().nth(1).unwrap();
            info!("get network id {}", result);
            let id = result.split_whitespace().nth(0).unwrap();

            Ok(id.to_string())
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::Failed,
                "docker ls network get id failed",
            ))
        }
        // Ok("".to_string())
    }

    pub fn get_network_gateway_ip() -> BuckyResult<String> {
        // docker network inspect --format='{{range .IPAM.Config}}{{.Gateway}}{{end}}' cyfs_br
        info!("get_network_id cmd: docker network inspect --format='{{range .IPAM.Config}}{{.Gateway}}{{end}}' {:?}", CYFS_BRIDGE_NAME);
        let output = Command::new("docker")
            .args([
                "network",
                "inspect",
                "--format='{{range .IPAM.Config}}{{.Gateway}}{{end}}'",
                CYFS_BRIDGE_NAME,
            ])
            .output()
            .map_err(|e| {
                error!("get_network_id cmd error {:?}", e);
                BuckyError::new(BuckyErrorCode::Failed, "get_network_id cmd error")
            })?;

        if output.status.success() {
            let stdout = String::from_utf8(output.stdout).unwrap();
            let result = stdout.lines().nth(0).unwrap();
            let ip = result
                .trim_start_matches("'")
                .trim_end_matches("'")
                .to_string();
            info!("get network Gateway ip {}", ip);
            Ok(ip)
        } else {
            Err(BuckyError::new(
                BuckyErrorCode::Failed,
                "docker ls network get Gateway ip failed",
            ))
        }
    }

    pub async fn install(
        &self,
        id: &str,
        version: &str,
        executable: Option<Vec<String>>,
    ) -> BuckyResult<()> {
        prepare(id, version, executable)?;
        self._install(id, version).await?;
        Ok(())
    }

    // 安装 镜像
    async fn _install(&self, id: &str, version: &str) -> BuckyResult<()> {
        info!("docker build image start {} {}", id, version);

        // build options
        let name = format!("decapp-{}:{}", id.to_lowercase(), version);
        let build_image_options = BuildImageOptions {
            dockerfile: "Dockerfile",
            t: &name,
            q: true,
            rm: true,
            forcerm: true,
            ..Default::default()
        };

        let dockerfile_gz_path = get_dockerfile_gz_path(&id, &version)?;
        let mut file = File::open(dockerfile_gz_path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).unwrap(); // 这里阻塞的去全量读取buffer可能有点性能问题，不过先不管了
        info!("docker build build_image_options {} {}", id, version);

        // docker build request
        let mut image_build_stream =
            self.docker
                .build_image(build_image_options, None, Some(contents.into()));
        info!("docker build request send {} {}", id, version);

        while let Some(msg) = image_build_stream.next().await {
            // info!("docker build Message: {:?}", msg);
            match msg {
                Ok(p) => {
                    info!("docker build Message: {:?}", p);
                }
                Err(e) => {
                    error!("docker build failed {} {} {}", id, version, e.to_string());
                    return Err(to_bucky_error(e));
                }
            }
        }

        info!("docker build finish {} {}", id, version);
        Ok(())
    }

    /// uninstall
    /// remove image, remove dockerfile.gz
    pub async fn uninstall(&self, id: &str) -> BuckyResult<()> {
        let _ = self._uninstall(id).await;
        let _ = self._remove_dockerfile(id).await;

        Ok(())
    }

    async fn _remove_dockerfile(&self, id: &str) -> BuckyResult<()> {
        let dockerfile_dir = get_app_dockerfile_dir(&id);
        if dockerfile_dir.exists() {
            std::fs::remove_dir_all(dockerfile_dir.clone())?;
        }
        Ok(())
    }

    async fn _uninstall(&self, id: &str) -> BuckyResult<()> {
        let image_name = self.get_image(id).await.unwrap();
        let remove_options = Some(bollard::image::RemoveImageOptions {
            force: true,
            ..Default::default()
        });
        self.docker
            .remove_image(&image_name, remove_options, None)
            .await
            .map_err(|e| {
                warn!("remove image failed, name:{}, err:{}", image_name, e);
                to_bucky_error(e)
            })?;

        Ok(())
    }

    /// get image full name(with tag)
    async fn get_image(&self, id: &str) -> BuckyResult<String> {
        let name = format!("decapp-{}", id.to_lowercase());
        let mut filters = HashMap::new();
        filters.insert("reference", vec![name.as_str()]);
        let options = Some(ListImagesOptions {
            all: true,
            filters: filters,
            ..Default::default()
        });
        let result = self.docker.list_images(options).await.unwrap();
        if result.len() > 0 {
            let image_with_tag = result[0].repo_tags[0].to_string();
            return Ok(image_with_tag);
        }

        Ok("".to_string())
    }

    // 运行容器
    pub async fn start(
        &self,
        id: &str,
        config: RunConfig,
        command: Option<Vec<String>>,
    ) -> BuckyResult<()> {
        info!("docker run dec app:{}, config {:?}", id, config);
        if self.is_running(id).await.unwrap() {
            info!("docker container is alreay running   {}", id);
            return Ok(());
        }
        // check image

        // build options
        let container_name = format!("decapp-{}", id.to_lowercase());
        let options = Some(CreateContainerOptions {
            name: container_name.clone(),
        });

        let image_name = self.get_image(id).await?;

        // 容器启动的host配置
        let mut host_config: HostConfig = HostConfig {
            network_mode: Some("none".to_string()),
            // network_mode: None,
            // 文件挂载处理
            mounts: get_hostconfig_mounts(id)?,
            // privileged: Some(true), // TOFIX unsafe: give true root to container.
            // 不通过privileged方式 来获取iptables配置权限
            // https://stackoverflow.com/a/44523905/4318885
            cap_add: Some(vec!["NET_ADMIN".to_string(), "NET_RAW".to_string()]),
            sysctls: Some(HashMap::from([(
                "net.ipv4.conf.eth0.route_localnet".to_string(),
                "1".to_string(),
            )])),
            // 容器的捕获的应用日志，需要限制一下size和数量，否则在一些小硬盘的机器上，会持续写爆硬盘
            log_config: Some(HostConfigLogConfig {
                typ: Some("json-file".to_string()),
                config: Some(HashMap::from([
                    ("max-size".to_string(), "100m".to_string()),
                    ("max-file".to_string(), "3".to_string()),
                ])),
            }),
            ..Default::default()
        };
        info!("docker run dec app:{}, host_config {:?}", id, host_config);

        // 内存限制
        if config.memory.is_some() {
            let memory = config.memory.unwrap() * 1048576;
            host_config.memory = Some(memory);
        }

        // cpu绝对限制
        if config.cpu_core.is_some() {
            let cpu_quota = (config.cpu_core.unwrap() * f64::from(100000)).round() as i64;
            host_config.cpu_period = Some(100000);
            host_config.cpu_quota = Some(cpu_quota);
        }

        // cpu相对限制
        if config.cpu_shares.is_some() {
            host_config.cpu_shares = config.cpu_shares;
        }

        let mut create_config = Config {
            image: Some(image_name),
            host_config: Some(host_config),
            cmd: command,
            ..Default::default()
        };

        // ip和network 配置
        // 通过docker network inspect cyfs_br 可以快速查看container的ip是否配置正确
        if config.ip.is_some() && config.network.is_some() {
            let mut endpoint_settings: HashMap<String, EndpointSettings> = HashMap::new();
            let network = config.network.unwrap();
            let network_id = DockerApi::get_network_id(&network)?;
            let ip = config.ip.clone().unwrap();
            info!(
                "docker network ls filter --> get network id: {}",
                network_id
            );
            endpoint_settings.insert(
                network,
                EndpointSettings {
                    ipam_config: Some(EndpointIpamConfig {
                        ipv4_address: Some(ip), // 配置由上层传入的 network ip
                        ..Default::default()
                    }),
                    network_id: Some(network_id),
                    ..Default::default()
                },
            );
            create_config.networking_config = Some(bollard::container::NetworkingConfig {
                endpoints_config: endpoint_settings,
            })
        }

        info!("dapp start: create container, {}", id);
        let run_result = self
            .docker
            .create_container(options, create_config)
            .await
            .err();
        if run_result.is_some() {
            if let Some(bollard::errors::Error::DockerResponseConflictError { .. }) = run_result {
            } else {
                info!("create container error, id:{}", id);
                return Err(to_bucky_error(run_result.unwrap()));
            }
        }

        info!("start_container , {}", id);
        let _resp = self
            .docker
            .start_container(&container_name, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| {
                warn!("start container failed, id:{}, err:{}", id, e);
                to_bucky_error(e)
            })?;

        Ok(())
    }

    pub async fn stop(&self, id: &str) -> BuckyResult<()> {
        let container_name = format!("decapp-{}", id.to_lowercase());
        info!("try to stop container[{}]", container_name);

        let mut filters = HashMap::new();
        filters.insert("name", vec![container_name.as_str()]);
        let options = Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        });

        // list container
        let result = self.docker.list_containers(options).await.map_err(|e| {
            warn!("list containers failed, err:{}", e);
            to_bucky_error(e)
        })?;
        info!("list container status result {:?}", result);
        if result.len() == 0 {
            info!("container[{:?}] not found", container_name);
            return Ok(());
        }

        let options = Some(StopContainerOptions { t: 30 });

        let _ = self.docker.stop_container(&container_name, options).await;

        let remove_options = Some(RemoveContainerOptions {
            v: false,
            force: true,
            ..Default::default()
        });
        self.docker
            .remove_container(&container_name, remove_options)
            .await
            .map_err(|e| {
                warn!("remove container failed, id:{}, err:{}", id, e);
                to_bucky_error(e)
            })?;
        Ok(())
    }

    pub async fn is_running(&self, id: &str) -> BuckyResult<bool> {
        let container_name = format!("decapp-{}", id.to_lowercase());

        let mut filters = HashMap::new();
        filters.insert("name", vec![container_name.as_str()]);
        let options = Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        });

        // list container
        let result = self.docker.list_containers(options).await.map_err(|e| {
            warn!("list containers failed, err:{}", e);
            to_bucky_error(e)
        })?;
        info!("check container status result {:?}", result);
        if result.len() == 0 {
            info!(
                "get list result, but container[{:?}] is not running ",
                container_name
            );
            return Ok(false);
        }

        let container_state = result[0].state.as_ref().unwrap().to_string();
        if container_state == "running".to_string() {
            info!("container[{}] is running ", container_name);
            return Ok(true);
        }

        info!("container[{}] is not running ", container_name);
        Ok(false)
    }

    pub async fn volume_create(&self, id: &str) -> BuckyResult<()> {
        let mut options = HashMap::new();
        //let volume_dir = format!("/cyfs/data/app/{}", id.clone());
        //options.insert("device", volume_dir.as_str());
        options.insert("device", "tmpfs");
        // options.insert("o", "size=10G"); // 小空间的机器会爆，这里要再想下
        options.insert("type", "tmpfs");

        let config = CreateVolumeOptions {
            name: id,
            driver: "local",
            driver_opts: options,
            ..Default::default()
        };
        //
        let _result = self.docker.create_volume(config).await.map_err(|e| {
            warn!("create volume failed, id: {}, err:{}", id, e);
            to_bucky_error(e)
        })?;
        info!("docker create volume finish: {}", id);
        Ok(())
    }

    pub async fn volume_remove(&self, id: &str) -> BuckyResult<()> {
        let options = RemoveVolumeOptions { force: true };

        self.docker
            .remove_volume(id, Some(options))
            .await
            .map_err(|e| {
                warn!("remove volume failed, id:{}, err:{}", id, e);
                to_bucky_error(e)
            })?;
        Ok(())
    }

    /// get_container_ip
    /// 通过 容器名字获取 容器的(nat)ip e.g. 172.17.0.2
    pub fn get_container_ip(&self, container_name: &str) -> BuckyResult<String> {
        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{ .NetworkSettings.IPAddress }}",
                container_name,
            ])
            .output()
            .unwrap();
        let ip = String::from_utf8(output.stdout).unwrap().replace("\n", "");
        Ok(ip)
    }
}

// bollard::errors::Error 类型转换,
// 在这个crate没办法增加一个impl BuckyError，所以先用函数转换
fn to_bucky_error(e: bollard::errors::Error) -> BuckyError {
    BuckyError::new(BuckyErrorCode::Unknown, format!("{}", e))
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

    /// dockers build 需要先 prepare
    #[async_std::test]
    async fn test_docker_image_build() {
        let docker_api = DockerApi::new();
        let resp = docker_api.install(id, version, None).await;

        // docker image ls ，然后正侧看一下有就 pass
        let output = Command::new("docker")
            .args(["image", "ls"])
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        info!("stdout {}", stdout);

        let name = format!("decapp-{}", id.to_lowercase());
        let re = regex::Regex::new(&name).unwrap();
        assert!(re.is_match(&stdout[..]));
    }

    #[async_std::test]
    async fn test_docker_image_remove() {
        let docker_api = DockerApi::new();
        let resp = docker_api.uninstall(id).await;

        let output = Command::new("docker")
            .args(["image", "ls"])
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        info!("stdout {}", stdout);

        let name = format!("decapp-{}", id.to_lowercase());
        let re = regex::Regex::new(&name).unwrap();
        assert!(!re.is_match(&stdout[..]));
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
                Some(vec!["tail -f /dev/null".to_string()]),
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
                Some(vec!["tail -f /dev/null".to_string()]),
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

        let output = Command::new("docker").args(["ps", "-a"]).output().unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();

        let mut is_exited = false;
        let mut lines = stdout.lines();
        let re = regex::Regex::new(&name).unwrap();
        while let Some(line) = lines.next() {
            if re.is_match(&line[..]) {
                is_exited = true;
            }
        }
        assert!(!is_exited);
    }

    #[async_std::test]
    async fn test_docker_container_is_running() {
        let docker_api = DockerApi::new();
        let is_running = docker_api.is_running(id).await.unwrap();

        info!("container is running: {:?}", is_running);
    }

    #[async_std::test]
    async fn test_docker_volume_create() {
        let docker_api = DockerApi::new();
        let resp = docker_api.volume_create(id).await;
        info!("docker create volume reslut {:?}", resp);

        let output = Command::new("docker")
            .args(["volume", "inspect", id])
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        info!("volume ls: {:?}", stdout);
    }

    #[async_std::test]
    async fn test_docker_volume_remove() {
        let docker_api = DockerApi::new();
        let resp = docker_api.volume_remove(id).await;

        let output = Command::new("docker")
            .args(["volume", "ls"])
            .output()
            .unwrap();
        info!("docker remove volume reslut {:?}", output)
    }

    #[async_std::test]
    async fn test_docker_get_image() {
        let docker_api = DockerApi::new();
        let resp = docker_api.get_image(id).await.unwrap();
        info!("docker get image with tag {:?}", resp);
    }

    #[async_std::test]
    async fn test_docker_get_container_ip() {
        let docker_api = DockerApi::new();
        let container_name = format!("decapp-{}", id.to_lowercase());

        let ip = docker_api.get_container_ip(&container_name).unwrap();
        info!("docker get container ip {:?}", ip);
    }
}
