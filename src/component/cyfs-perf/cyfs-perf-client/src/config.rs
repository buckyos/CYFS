use cyfs_base::*;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};

use std::convert::TryFrom;
use std::convert::TryInto;

// perf_service在链上的名字
pub const CYFS_PERF_SERVICE_NAME: &str = "cyfs-perf-service";

pub(crate) struct PerfServerLoader {}

#[derive(Clone, Debug)]
pub enum PerfServerConfig {
    // 上报到所在zone的ood
    OOD,

    // 上报到指定的目标设备
    Specified(DeviceId),

    // 系统默认值，优先使用本地配置，其次使用链上的值
    Default,
}

impl Default for PerfServerConfig {
    fn default() -> Self {
        Self::Default
    }
}

impl PerfServerLoader {
    // 根据配置初始化目标perf_server
    pub async fn load_perf_server(target_type: PerfServerConfig) -> Option<DeviceId> {
        match target_type {
            PerfServerConfig::OOD => None,
            PerfServerConfig::Specified(device_id) => Some(device_id),
            PerfServerConfig::Default => Self::load_default_perf_server().await,
        }
    }

    async fn load_default_perf_server() -> Option<DeviceId> {
        if let Ok(Some(id)) = Self::load_perf_server_from_file() {
            return Some(id);
        }

        if let Ok(Some(id)) = Self::load_perf_server_from_meta().await {
            return Some(id);
        }

        error!("none of perf server config founded!, will use empty perf server");

        None
    }

    // 从磁盘加载
    fn load_perf_server_from_file() -> BuckyResult<Option<DeviceId>> {
        let base_path = cyfs_util::get_cyfs_root_path();

        let desc_file_name = "perf.desc";
        let dir_path = base_path.join("etc").join("desc");

        let desc_file_path = dir_path.join(desc_file_name);

        if !desc_file_path.exists() {
            warn!("perf desc file not found: {}", desc_file_path.display());
            return Ok(None);
        }

        let (obj, _) =
            AnyNamedObject::decode_from_file(&desc_file_path, &mut vec![]).map_err(|e| {
                let msg = format!(
                    "invalid perf server desc file format: file={}, {}",
                    desc_file_path.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;
        let object_id = obj.object_id();
        match obj {
            AnyNamedObject::Standard(StandardObject::Device(_)) => {
                let device_id = object_id.try_into().unwrap();
                info!(
                    "load perf server from desc file success! file={}, target={}",
                    desc_file_path.display(),
                    device_id
                );
                Ok(Some(device_id))
            }
            _ => {
                error!(
                    "invalid perf server desc file object type: file={}, id={}, obj_type={}",
                    desc_file_path.display(),
                    object_id,
                    obj.obj_type(),
                );
                Ok(None)
            }
        }
    }

    async fn load_perf_server_from_meta() -> BuckyResult<Option<DeviceId>> {
        // 查找perf_service的DeviceId
        let meta_client = MetaClient::new_target(MetaMinerTarget::default());
        if let Some((info, _state)) = meta_client.get_name(CYFS_PERF_SERVICE_NAME).await? {
            if let NameLink::ObjectLink(obj) = info.record.link {
                let device_id = DeviceId::try_from(obj)?;
                info!(
                    "find perf_service from meta: chain={:?}, id={}",
                    MetaMinerTarget::default(),
                    device_id
                );
                Ok(Some(device_id))
            } else {
                info!(
                    "object link not match perf_service: chain={:?}, record={:?}",
                    MetaMinerTarget::default(),
                    info.record
                );
                Err(BuckyError::from(BuckyErrorCode::NotMatch))
            }
        } else {
            info!(
                "perf_service not found on meta chain: chain={:?}, name={}",
                MetaMinerTarget::default(),
                CYFS_PERF_SERVICE_NAME
            );
            Ok(None)
        }
    }
}
