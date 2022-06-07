use cyfs_base::*;
use cyfs_core::DecAppId;
use log::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::sync::RwLock;

pub const CYFS_BRIDGE_NAME: &str = "cyfs_br";
const MAX_IP_COUNT: u64 = 255 * 255;

pub struct DockerNetworkManager {
    subnet_segment: RwLock<String>, //172.xx.0.1  xx网段
}

impl DockerNetworkManager {
    pub fn new() -> Self {
        Self {
            subnet_segment: RwLock::new("".to_owned()),
        }
    }

    pub fn init(&self) -> BuckyResult<()> {
        let ip_segment;
        match Self::get_sub_net_segment() {
            Ok(v) => {
                ip_segment = v;
            }
            Err(e) => {
                info!("get sub net failed. err: {}", e);
                ip_segment = Self::create_netbridge()?;
                // if e.code() == BuckyErrorCode::NotFound {
                //     ip_segment = Self::create_netbridge()?;
                // } else {
                //     return Err(e);
                // }
            }
        }

        info!("docker gateway ip:{}", format!("172.{}.0.1", ip_segment));

        *self.subnet_segment.write().unwrap() = ip_segment;

        Ok(())
    }

    pub fn gateway_ip(&self) -> String {
        let subnet_segment = self.subnet_segment.read().unwrap();
        let gateway = format!("172.{}.0.1", subnet_segment);
        gateway
    }

    pub fn get_valid_app_ip(&self, app_id: &DecAppId) -> BuckyResult<String> {
        let mut id_hash = Self::calculate_hash(app_id);
        let mut try_count = 0;
        let subnet_segment = self.subnet_segment.read().unwrap();

        loop {
            let ip = format!("172.{}.{}.{}", subnet_segment, id_hash / 255, id_hash % 255);

            if !Self::is_ip_in_use(&ip) {
                info!("find a valid ip for app:{}, ip:{}", app_id, ip);
                return Ok(ip);
            }

            info!("ip[{}] is in use, will try another", ip);

            id_hash = id_hash + 1;
            try_count = try_count + 1;

            if id_hash >= MAX_IP_COUNT {
                //从0.2开始
                id_hash = 2;
            }
            if try_count > MAX_IP_COUNT {
                break;
            }
        }

        Err(BuckyError::from(BuckyErrorCode::OutOfLimit))
    }

    fn is_ip_in_use(ip: &str) -> bool {
        let mut cmd = "docker inspect --format='{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $(docker ps -q) | grep ".to_string();
        cmd += ip;
        match Command::new("bash").arg("-c").arg(cmd).output() {
            Ok(output) => {
                if let Ok(out) = String::from_utf8(output.stdout) {
                    if out.len() == 0 {
                        return false;
                    }
                }
            }
            Err(e) => {
                //不冒险，失败就认为在使用了
                warn!("test ip in use failed, err: {}", e);
            }
        }

        true
    }

    fn get_sub_net_segment() -> BuckyResult<String> {
        let output = Command::new("docker")
            .args([
                "network",
                "inspect",
                "--format='{{range .IPAM.Config}}{{.Subnet}}{{end}}'",
                CYFS_BRIDGE_NAME,
            ])
            .output()?;

        if output.status.success() {
            let subnet = String::from_utf8(output.stdout).map_err(|e| {
                error!("convert query subnet output failed, err:{:?}", e);
                BuckyError::from(BuckyErrorCode::ParseError)
            })?;
            info!("get subnet success: {}", subnet);
            if subnet.len() > 0 {
                let split = subnet.split(".");
                let vec: Vec<&str> = split.collect();
                if vec.len() == 4 {
                    return Ok(vec[1].to_string());
                }
            } else {
                info!("network {} is not found", CYFS_BRIDGE_NAME);
                return Err(BuckyError::from(BuckyErrorCode::NotFound));
            }
        } else {
            warn!("get sub net failed, execute cmd failed, {:?}", output);
        }

        Err(BuckyError::from(BuckyErrorCode::Failed))
    }

    fn create_netbridge() -> BuckyResult<String> {
        info!("will create netbridge");
        //docker network create --subnet=172.20.0.0/16 --gateway=172.20.0.1 cyfs_br1
        //循环创建，直到成功为止，理论上有255*255-2个地址可用
        let mut ip_segment = 19;
        loop {
            let subnet = format!("--subnet=172.{}.0.0/16", ip_segment);
            //约定.0.1一定是gateway
            let gateway = format!("--gateway=172.{}.0.1", ip_segment);
            if Command::new("docker")
                .args(["network", "create", &subnet, &gateway, CYFS_BRIDGE_NAME])
                .output()?
                .status
                .success()
            {
                info!(
                    "create netbridge success, ip section: subnet: 172.{}.0.0/16",
                    ip_segment
                );
                let ip_segment_str = format!("{}", ip_segment);
                return Ok(ip_segment_str);
            }
            ip_segment += 1;
            if ip_segment > 255 {
                error!("create netbridge failed. ip segment > 255");
                return Err(BuckyError::from(BuckyErrorCode::Failed));
            }
        }
    }

    //根据appid计算hash，
    fn calculate_hash<T: Hash>(t: &T) -> u64 {
        let mut s = DefaultHasher::new();
        t.hash(&mut s);
        let mut v = s.finish() % MAX_IP_COUNT;
        //0.0是虚拟网卡，0.1是gateway，只能从0.2开始
        if v < 2 {
            v = 2;
        }
        v
    }
}
