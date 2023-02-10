use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, BuckyError, BuckyErrorCode, Device, FileEncoder};
use crate::def::MonitorRunner;
use crate::SERVICE_NAME;

pub struct SNOnlineMonitor {
    sn: Device,
    name: String
}

impl SNOnlineMonitor {
    pub(crate) fn new(sn: Device) -> Self {
        let name = format!("sn online test {}", sn.desc().calculate_id());
        Self {
            sn,
            name
        }
    }
}

const NAME: &str = "sn_online_check";

#[async_trait::async_trait]
impl MonitorRunner for SNOnlineMonitor {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run_once(&self, _once: bool) -> BuckyResult<()> {
        let mut test_prog = std::env::current_exe().unwrap().parent().unwrap().join("sn-online-test");
        #[cfg(windows)]
        {
            test_prog = test_prog.with_extension("exe");
        }


        if !test_prog.exists() {
            return Err(BuckyError::new(BuckyErrorCode::NotFound, format!("sn-online-test program not found!")));
        }
        let sn_path = cyfs_util::get_service_data_dir(SERVICE_NAME)
            .join("sn-desc")
            .join(self.sn.desc().calculate_id().to_string())
            .with_extension("desc");
        std::fs::create_dir_all(sn_path.parent().unwrap()).unwrap();
        self.sn.encode_to_file(&sn_path, false)?;

        // 用参数再启动一次，做真正的测试
        let mut command = async_std::process::Command::new(test_prog);
        command.arg(sn_path);
        let child = command.spawn()?;
        let id = child.id();
        let out = child.output().await?;
        info!("run seperate sn test pid {} status {}", id, out.status);
        if out.status.success() {
            Ok(())
        } else {
            Err(BuckyError::from(BuckyErrorCode::from(out.status.code().unwrap_or(1) as u32)))
        }
    }
}