use serde::{Deserialize};
//use cyfs_base::ObjectId;

#[derive(Deserialize)]
pub struct Config {
    pub run_times: Option<usize>,
    pub same_zone_target: Option<String>,
    pub cross_zone_target: Option<String>,
    pub http_port: u16,
    pub ws_port: u16
}

impl Config {
    pub fn simulator() -> Self {
        Self {
            run_times: None,
            same_zone_target: None,
            cross_zone_target: None,
            http_port: 21002,
            ws_port: 21003
        }
    }
}