use std::str::FromStr;
use once_cell::sync::OnceCell;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ObjectId, RawFrom};
use cyfs_client::NamedCacheClient;
use cyfs_core::{AppList, APPLIST_SERVICE_CATEGORY, AppListObj, AppStatusObj, DecAppId, DecApp, DecAppObj};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use crate::def::MonitorRunner;

pub struct CyfsRepoMonitor {
    meta_client: MetaClient,
    cyfs_client: OnceCell<NamedCacheClient>,
}

const DEVICE_DESC: &str = "00015202002f3def2effc3e10000000002010030818902818100e348de239de3941cca20b864f8cbe6b74e76bc28a375a9151b274cfa5eabba4bcdbd2eb1e5e7d692e971acb4d9b6ff2165e6327a7e099ad256fa0cafee37060cab9d66e03d1a125f8796ac2ee3addf8d38a0c01238acd8aca68af80cbdd0569b7808a293afe10412295a1307c40f34bb5de26d4aaa9376b175cb0d0f5bc2eff702030100010000000000000000000000000000000000000000000000000010746573742d637966732d7265706f000000002f3def2effc3e1000100";
const DEVICE_SEC: &str = "0002623082025e02010002818100e348de239de3941cca20b864f8cbe6b74e76bc28a375a9151b274cfa5eabba4bcdbd2eb1e5e7d692e971acb4d9b6ff2165e6327a7e099ad256fa0cafee37060cab9d66e03d1a125f8796ac2ee3addf8d38a0c01238acd8aca68af80cbdd0569b7808a293afe10412295a1307c40f34bb5de26d4aaa9376b175cb0d0f5bc2eff7020301000102818100ab1a3b2902fec58cdad9c1173a797de9b76709856a70f466103808ea5f04d6cda447ec743e88c6ef78507c5cf59d9ef9cc957ca0dc6b6ca336992d9df02e7a1cfa43ef94f3537c4ef3b2330b0692ff5590a7b247d67f2c732533d09c20847c34b5aa2566fa325f4d587446e97188475493f28092fadfe6dbe19aa55d76b3e2b1024100fb49bdcb1e449da78c43ebd47ae81f34d7de76a8194f0117545c4b74f7deb8d65d0797911312e7a05c3d1ca7671c4a478edc92f36c46f5596d404141a835bc5f024100e78be71830005539be495c6a6734905c8770aacf9096117a71e0307773957d92893c5b1a9b8d84af1266b42b32406d489af296df9ba40eac258b7b2f3c26736902407ee072c5d5d88b498796dbc202f4a49d07c9b95b92bbc32f4656fb7a6994b8faf329dc2b51d81fbf66132d1e90ff45b9efb60b34811d2ad0264b65278388ee3d024100bf0867f346b71f99726b283a09480ecaa85bc63155c2da4cc1630bd9a19cf66b4d9a6437c19ab29b967cf1aca9db09cedb37c64e5a24b28e48b39940514a0ff1024100cdbe6dbf3cb707074868a84128e6b46b72b10a2a2501462a69a2b6f21cff21cdcb0df2737ecc4589025ffa79b3189b3682f5b59f7cda49ea109ddb321e728ebe";
const SERVICE_REPO: &str = "5aSixgLzAmyR5QbQibWFkrkNbBLagfawmK3pbdaYqyt6";
const DAEMON_APP_ID: &str = "9tGpLNnTdsycFPRcpBNgK1qncX6Mh8chRLK28mhNb6fU";

impl CyfsRepoMonitor {
    pub(crate) fn new() -> Self {
        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            cyfs_client: OnceCell::new()
        }
    }
}

#[async_trait::async_trait]
impl MonitorRunner for CyfsRepoMonitor {
    fn name(&self) -> &str {
        "cyfs-repo"
    }

    async fn run_once(&self, _: bool) -> BuckyResult<()> {
        // 1. 通过meta-client查询链上的service-list文件
        let list_id = AppList::generate_id(ObjectId::from_str(SERVICE_REPO)?, "nightly", APPLIST_SERVICE_CATEGORY);
        let ret = self.meta_client.get_raw_data(&list_id).await?;
        let list = AppList::clone_from_slice(&ret)?;
        // 2. 识别ood-daemon的最新版本
        let daemon_id = DecAppId::from_str(DAEMON_APP_ID)?;
        if let Some(status) = list.app_list().get(&daemon_id) {
            let app_raw = self.meta_client.get_raw_data(daemon_id.object_id()).await?;
            let app = DecApp::clone_from_slice(&app_raw)?;
            let file_id = app.find_source(status.version())?;

            // 3. 通过cyfs-client下载这个版本
            if self.cyfs_client.get().is_none() {
                let mut client = NamedCacheClient::new();
                let device = cyfs_base::Device::clone_from_hex(DEVICE_DESC, &mut vec![])?;
                let secret = cyfs_base::PrivateKey::clone_from_hex(DEVICE_SEC, &mut vec![])?;
                client.init(Some(device), Some(secret), None, None).await?;
                self.cyfs_client.set(client);
            }

            let client = self.cyfs_client.get().unwrap();
            let tmp_path = std::env::temp_dir().join(file_id.to_string());
            client.get_dir_by_obj(&file_id, None, Some("x86_64-pc-windows-msvc.zip"), &tmp_path).await?;

            std::fs::remove_file(tmp_path)?;
        } else {
            let msg = format!("ood-daemon status not found from list {}, appid {}", &list_id, &daemon_id);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        Ok(())
    }
}