use crate::panic::CyfsPanicInfo;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PanicReportRequest {
    pub product_name: String,
    pub service_name: String,
    pub exe_name: String,
    pub target: String,
    pub version: String,
    pub channel: String,

    pub info: CyfsPanicInfo,
}

impl PanicReportRequest {
    pub fn new(product_name: &str, service_name: &str, panic_info: CyfsPanicInfo) -> Self {
        let exe_name = match std::env::current_exe() {
            Ok(path) => match path.file_name() {
                Some(v) => v.to_str().unwrap_or("[unknown]").to_owned(),
                None => "[unknown]".to_owned(),
            },
            Err(_e) => "[unknown]".to_owned(),
        };

        Self {
            product_name: product_name.to_owned(),
            service_name: service_name.to_owned(),
            exe_name,
            target: cyfs_base::get_target().to_owned(),
            version: cyfs_base::get_version().to_owned(),
            channel: cyfs_base::get_channel().to_string(),
            info: panic_info,
        }
    }

    pub fn to_string(&self) -> String {
        match serde_json::to_string(&self) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("encode panic request to string error: {:?}, {}", self, e);
                error!("{}", msg);
                msg
            }
        }
    }

    pub fn info_to_string(&self) -> String {
        match serde_json::to_string(&self.info) {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("encode panic info to string error: {:?}, {}", self.info, e);
                error!("{}", msg);
                msg
            }
        }
    }
}
