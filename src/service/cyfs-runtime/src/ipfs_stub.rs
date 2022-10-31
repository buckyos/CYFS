use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use async_std::process::Child;

pub struct IPFSStub {
    ipfs_data_path: PathBuf,
    ipfs_prog: PathBuf,
}

#[cfg(target_os = "windows")]
const DEFAULT_IPFS_PROG_NAME: &str = "ipfs.exe";

#[cfg(not(target_os = "windows"))]
const DEFAULT_IPFS_PROG_NAME: &str = "ipfs";

impl IPFSStub {
    // 构造函数
    // ipfs_data_path： IPFS程序的data dir目录位置
    // ipfs_prog: ipfs可执行程序的位置
    pub fn new(ipfs_data_path: Option<&Path>, ipfs_prog_path: Option<&Path>, ipfs_prog_name: Option<&str>) -> Self {
        let ipfs_data_path = if let Some(path) = ipfs_data_path {
            PathBuf::from(path)
        } else {
            cyfs_util::get_service_data_dir("ipfs")
        };

        let cur_exe_path = std::env::current_exe().unwrap();
        let ipfs_prog = if let Some(path) = ipfs_prog_path {
            path
        } else {
            cur_exe_path.parent().unwrap()
        }.join(ipfs_prog_name.unwrap_or(DEFAULT_IPFS_PROG_NAME));

        Self {
            ipfs_data_path,
            ipfs_prog,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.ipfs_prog.exists()
    }

    pub fn ipfs_prog_path(&self) -> &Path {
        &self.ipfs_prog
    }

    pub async fn init(&self, ipfs_gateway_port: u16, ipfs_api_port: u16, swarm_port: u16) -> BuckyResult<()> {
        let _ = self.run_ipfs(&vec!["init".as_ref()])?.status().await?;
        let _ = self.run_ipfs(&vec!["config".as_ref(), "Addresses.Gateway".as_ref(), &format!("/ip4/127.0.0.1/tcp/{}", ipfs_gateway_port).as_ref()])?.status().await?;
        let _ = self.run_ipfs(&vec!["config".as_ref(), "Addresses.API".as_ref(), &format!("/ip4/127.0.0.1/tcp/{}", ipfs_api_port).as_ref()])?.status().await?;
        let _ = self.run_ipfs(&vec!["config".as_ref(), "Addresses.Swarm".as_ref(), "--json".as_ref(), &serde_json::to_string(&vec![
            format!("/ip4/0.0.0.0/tcp/{}", swarm_port),
            format!("/ip6/::/tcp/{}", swarm_port),
            format!("/ip4/0.0.0.0/udp/{}/quic", swarm_port),
            format!("/ip6/::/udp/{}/quic", swarm_port)]).unwrap().as_ref()
        ])?.status().await?;

        Ok(())
    }

    pub async fn is_init(&self) -> bool {
        if let Ok(mut child) = self.run_ipfs(&vec!["id".as_ref()]) {
            if let Ok(status) = child.status().await {
                status.success()
            } else {
                false
            }
        } else {
            false
        }
    }

    // 启动ipfs daemon，返回启动成功或者失败即可
    pub async fn start(&self) -> bool {
        if self.ipfs_is_running().await {
            info!("ipfs daemon running");
            return true;
        }

        if let Ok(_) = self.run_ipfs(&vec!["daemon".as_ref()]) {
            for _ in [0..2] {
                info!("waiting ipfs daemon run...");
                async_std::task::sleep(Duration::from_secs(3)).await;
                if self.ipfs_is_running().await {
                    info!("ipfs daemon running");
                    return true;
                }
            }
            error!("ipfs daemon not running after 9 secs, start falied");
            false
        } else {
            false
        }
    }

    // 返回默认的ipns key。我们不支持第二个
    pub async fn get_ipns_key(&self) -> BuckyResult<String> {
        let child = self.run_ipfs(&vec!["id".as_ref(), "-f".as_ref(), "\"<id>\"".as_ref(), "--peerid-base".as_ref(), "base36".as_ref()])?;
        let output = child.output().await?;
        if !output.status.success() {
            Err(BuckyError::new(BuckyErrorCode::Failed, String::from_utf8_lossy(&output.stderr)))
        } else {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
    }

    // path: 一个文件或文件夹的路径
    // 返回这个路径发布后的CID。如果路径是文件夹，则只返回这个文件夹本身的CID
    pub async fn add(&self, path: &Path) -> BuckyResult<String> {
        let child = self.run_ipfs(&vec!["add".as_ref(), "-r".as_ref(), path.as_os_str()])?;
        let output = child.output().await?;
        if !output.status.success() {
            return Err(BuckyError::new(BuckyErrorCode::Failed, String::from_utf8_lossy(&output.stderr)))
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.split("\n").collect();

            let mut i = lines.len() as isize;
            i -= 2;

            if i >= 0 {
                let line = lines.get(i as usize).unwrap();

                let row: Vec<&str> = line.split(" ").collect();
                if row.len() < 3 {
                    return Err(BuckyError::new(BuckyErrorCode::Failed, format!("parse result err: {}", stdout)))
                }

                let cid = row.get(1).unwrap().to_string();

                return Ok(cid);
            }
        }

        Err(BuckyError::new(BuckyErrorCode::Failed, format!("unmatch any file cid")))
    }

    // 绑定CID到默认的ipns key，并返回这个key
    pub async fn bind_ipns(&self, cid: &str) -> BuckyResult<String> {
        let child = self.run_ipfs(&vec!["name".as_ref(), "publish".as_ref(), cid.as_ref()])?;
        let output = child.output().await?;
        if !output.status.success() {
            Err(BuckyError::new(BuckyErrorCode::Failed, String::from_utf8_lossy(&output.stderr)))
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let row: Vec<&str> = stdout.split(" ").collect();
            if row.len() < 4 {
                return Err(BuckyError::new(BuckyErrorCode::Failed, format!("parse result err: {}", stdout)))
            }

            let ipns = row.get(2).unwrap().to_string();
            let ret = ipns.replace(":", "");

            Ok(ret)
        }
    }

    pub async fn stop_ipfs(&self) {
        if let Ok(mut child) = self.run_ipfs(&vec!["shutdown".as_ref()]) {
            let _ = child.status().await;
        }
    }

    async fn ipfs_is_running(&self) -> bool {
        if let Ok(mut child) = self.run_ipfs(&vec!["swarm".as_ref(), "addrs".as_ref()]) {
            if let Ok(status) = child.status().await {
                status.success()
            } else {
                false
            }
        } else {
            false
        }
    }

    fn run_ipfs(&self, args: &Vec<&OsStr>) -> BuckyResult<Child> {
        let mut cmd = async_std::process::Command::new(&self.ipfs_prog);
        let mut real_args = vec!["--repo-dir".as_ref(), self.ipfs_data_path.as_os_str()];
        //let mut old_args = args.clone();
        real_args.extend(args);
        cmd.args(&real_args);
        info!("spawn ipfs path {} args {}", self.ipfs_prog.display(), real_args.join(" ".as_ref()).to_string_lossy());

        return Ok(cmd.spawn().map_err(|e| {
            error!("spawn ipfs path {} args {} err {}", self.ipfs_prog.display(), real_args.join(" ".as_ref()).to_string_lossy(), e);
            e
        })?);
    }
}