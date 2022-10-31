use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, RawFrom, BuckyError, BuckyErrorCode, Endpoint, Protocol};
use rand::Rng;
use crate::def::MonitorRunner;


pub struct SNOnlineMonitor {
}

const DEVICE_DESC: &str = "00015202002f3deda2ecf8a80000000001010030818902818100d725ee97d7656e2aeef64237750f422de18e0dbec504b9357415d07db967b53221c90a8444c61cadab666fb2770ef6d73aef0788ba3dc4a0050b257cf7bbe6f23347d4c638ec9c6e707d6e475fb8b955ec1c46ae54bbc5266a6a1759bdb7722474ad259a76773bab7df872084e915eaf0e1e9c1619c6a132b8b6d84caf831c4d020301000100000000000000000000000000000000000000000000000000103132330000000000000000000000000000002f3deda2ecf8a8000100";
const DEVICE_SEC: &str ="0002603082025c02010002818100d725ee97d7656e2aeef64237750f422de18e0dbec504b9357415d07db967b53221c90a8444c61cadab666fb2770ef6d73aef0788ba3dc4a0050b257cf7bbe6f23347d4c638ec9c6e707d6e475fb8b955ec1c46ae54bbc5266a6a1759bdb7722474ad259a76773bab7df872084e915eaf0e1e9c1619c6a132b8b6d84caf831c4d02030100010281807ce997072d95c44ac506d11725adc03ca73234a4c7faa0157ada977c0743411e64233729e84c25a66757480e93b56a7737ce43cf8c620460ebccb6ed4160250af482fabf7789475f04409125294f689d05ca08df2f18923e177f94cbdc56bf1e971f8bf9e4748b7e3df6bf5a38f4ac1f3725fdf1df9e1abac2d982ad07371281024100fde1b9c5ba3a4301581732be53755a923dd8022145d9a2f966930a95189e749355b93811ffac09e6e618bbb8dca4b5b92f8a511c76942a8137bb3f2f81e1f531024100d8f1795a2d56393cca7711ee68a276ceabeb104b11fad7d72e59845e5f3091aaf7dcbfd03c3e6b15ca504d8c8bea569c4fb58221d1ea9b3be6c3129e2b0141dd024067eb9fa94a03532e17aad74084d5028fddf4af5a834704a8e5cdc6852520a7432fd1b31bdaf5c6cfd6dbc4eb74958f35103aa3dcecc4d5693330d83a5005f7e10240591933a1d9a4e3f517a2377716fa429936fa5fc2b52bb4a1e7a3543dfe1250814b331a844779cb3933d22f475ddf6c9ade11c9d462065ca3096f6ca2113f7ef1024100887e522be730f317a4e1c1eedb8012facbfeb80232a4ea5ab6ba6debc3807da0085cacd4c7e7e873ac84cb4fe877d0b7204dc22a9d91d15d7a56ee6505aca1fc";

impl SNOnlineMonitor {
    pub(crate) fn new() -> Self {
        Self {
        }
    }
}

const NAME: &str = "sn_online_check";

#[async_trait::async_trait]
impl MonitorRunner for SNOnlineMonitor {
    fn name(&self) -> &str {
        NAME
    }

    async fn run_once(&self, once: bool) -> BuckyResult<()> {
        if once {
            // 测试起一个单独的bdt栈，等待它上线
            let mut device = cyfs_base::Device::clone_from_hex(DEVICE_DESC, &mut vec![])?;
            let secret = cyfs_base::PrivateKey::clone_from_hex(DEVICE_SEC, &mut vec![])?;
            let device_id = device.desc().calculate_id();
            info!("current device_id: {}", device_id);

            // desc.endpoints.clear();
            let endpoints = device.body_mut().as_mut().unwrap().content_mut().mut_endpoints();
            if endpoints.len() == 0 {
                // 取随机端口号
                let port = rand::thread_rng().gen_range(30000, 50000) as u16;
                for ip in cyfs_util::get_all_ips().unwrap() {
                    if ip.is_ipv4() {
                        endpoints.push(Endpoint::from((Protocol::Tcp, ip, port)));
                        endpoints.push(Endpoint::from((Protocol::Udp, ip, port)));
                    }
                }
            }

            //TODO:需要的时候可以选择和gateway用同一个bdt stack
            let mut init_sn_peers = vec![];
            let sn = cyfs_util::get_default_sn_desc();
            device.body_mut().as_mut().unwrap().content_mut().mut_sn_list().push(sn.desc().device_id());
            init_sn_peers.push(sn);

            let init_known_peers = cyfs_util::get_default_known_peers();
            let mut params = cyfs_bdt::StackOpenParams::new("cyfs-monitor");
            params.known_sn = Some(init_sn_peers);
            params.known_device = Some(init_known_peers);

            let desc = device.clone();
            let secret = secret.clone();
            let bdt_stack = cyfs_bdt::Stack::open(desc,secret, params).await?;

            // 创建bdt-stack协议栈后等待bdt-stack在SN上线
            info!(
                "now will wait for sn online {}......",
                bdt_stack.local_device_id()
            );
            let begin = std::time::Instant::now();
            let net_listener = bdt_stack.net_manager().listener().clone();
            let ret = net_listener.wait_online().await;
            let during = std::time::Instant::now() - begin;
            if let Err(e) = ret {
                let msg = format!(
                    "bdt stack wait sn online failed! {}, during={}s, {}",
                    bdt_stack.local_device_id(),
                    during.as_secs(),
                    e
                );
                return Err(BuckyError::new(BuckyErrorCode::ConnectFailed, msg));

            } else {
                info!(
                "bdt stack sn online success! {}, during={}s",
                bdt_stack.local_device_id(),
                during.as_secs()
            );
            }

            Ok(())
        } else {
            // 用参数再启动一次，做真正的测试
            let exe = std::env::current_exe()?;
            let mut child = std::process::Command::new(exe)
                .arg(NAME)
                .spawn()?;
            let status = child.wait()?;
            info!("run seperate sn test pid {} status {}", child.id(), status);
            if status.success() {
                Ok(())
            } else {
                Err(BuckyError::from(BuckyErrorCode::from(status.code().unwrap_or(1) as u32)))
            }
        }
    }
}