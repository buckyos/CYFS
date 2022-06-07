use cyfs_base::*;

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct ZoneData {
    pub mnemonic: String,

    // zone索引，目前支持0，1
    pub zone_index: u8,

    // device设备类型，分为ood和device
    pub device_type: String,

    // zone内的device索引，每个zone支持两个device
    pub device_index: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileData {
    // 使用的zone信息
    pub zone: Option<ZoneData>,

    // 端口的一些配置
    pub ws: bool,
    pub bdt_port: u16,
    pub service_port: u16,
}

pub struct Profile {
    pub data: ProfileData,
    pub config_file: PathBuf,
}

impl Profile {
    pub fn load() -> BuckyResult<Self> {
        let root = std::env::current_exe().unwrap();
        let file = root.join("../config.toml").canonicalize().unwrap();

        let s = std::fs::read_to_string(&file).map_err(|e| {
            let msg = format!(
                "load config file to string error! file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("will load config: {}", s);

        let data: ProfileData = ::toml::from_str(&s).map_err(|e| {
            let msg = format!("load config as toml error! value={}, err={}", s, e);
            error!("{}", msg);
            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        // 检查zone data有效性
        if let Some(zone_data) = &data.zone {
            Self::check_zone(zone_data)?;
        }

        let ret = Self {
            data,
            config_file: file,
        };

        Ok(ret)
    }

    fn check_zone(data: &ZoneData) -> BuckyResult<()> {
        cyfs_cip::CyfsSeedKeyBip::fix_mnemonic(&data.mnemonic)?;
        zone_simulator::TEST_PROFILE.set_mnemonic(&data.mnemonic);

        if data.zone_index > 1 {
            let msg = format!("zone_index must be 0 or 1!");
            error!("{}", msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        if data.device_type != "ood" && data.device_type != "device" {
            let msg = format!("device_type must be 'ood' or 'device'!");
            error!("{}", msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        if data.device_index > 1 {
            let msg = format!("device_index must be 0 or 1!");
            error!("{}", msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        Ok(())
    }
}
