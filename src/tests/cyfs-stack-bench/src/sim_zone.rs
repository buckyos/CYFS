use std::str::FromStr;

use cyfs_base::ObjectId;
use cyfs_lib::*;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Param {
	pub name: String,
	pub bdt_port: u16,
	pub http_port: u16,
    pub ws_port: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ZoneData {
	pub name: String,
	pub zone1: Vec<Param>,
	pub zone2: Vec<Param>,
}
#[derive(Clone)]
pub struct SimZone {
    zone1_ood_objectid: String,
    zone1_standby_ood_objectid: String,
    zone1_people: String,
    zone1_device1_objectid: String,
    zone1_device2_objectid: String,

    zone2_ood_objectid: String,
    zone2_device1_objectid: String,
    zone2_device2_objectid: String,
    zone2_people: String,

    zone1_ood_stack: SharedCyfsStack,
    zone1_device1_stack: SharedCyfsStack,
    zone1_device2_stack: SharedCyfsStack,
    zone1_standby_ood_stack: SharedCyfsStack,

    zone2_ood_stack: SharedCyfsStack,
    zone2_device1_stack: SharedCyfsStack,
    zone2_device2_stack: SharedCyfsStack,
}

impl SimZone {
    pub async fn init_zone() -> SimZone {
        let s = r#"{
            "name": "zone_simulator",
            "zone1": [
                {
                    "name": "zone1_ood",
                    "bdt_port": 20001,
                    "http_port": 21000,
                    "ws_port": 21001
                },
                {
                    "name": "zone1_device1",
                    "bdt_port": 20002,
                    "http_port": 21002,
                    "ws_port": 21003
                },
                {
                    "name": "zone1_device2",
                    "bdt_port": 20003,
                    "http_port": 21004,
                    "ws_port": 21005
                },
                {
                    "name": "zone1_standby_ood",
                    "bdt_port": 20004,
                    "http_port": 21006,
                    "ws_port": 21007
                }
            ],
            "zone2": [
                {
                    "name": "zone2_ood",
                    "bdt_port": 20010,
                    "http_port": 21010,
                    "ws_port": 21011
                },
                {
                    "name": "zone2_device1",
                    "bdt_port": 20011,
                    "http_port": 21012,
                    "ws_port": 21013
                },
                {
                    "name": "zone2_device2",
                    "bdt_port": 20012,
                    "http_port": 21014,
                    "ws_port": 21015
                }
            ]
        }"#;
    
        let zone: ZoneData = serde_json::from_str(s).unwrap();
        trace!("{:?}", zone);
    
        let root = ::cyfs_util::get_cyfs_root_path().join("etc").join("zone-simulator");
        let desc_list_file_path = root.join("desc_list");
        if !desc_list_file_path.exists() {
            error!("desc_list not existed! dir={}", desc_list_file_path.display());
        }
        let content = std::fs::read_to_string(desc_list_file_path).unwrap();
        let pos :Vec<&str> = content.split("\n").collect();
        trace!("desc_list: {:?}", pos);
    
        let zone1_people :Vec<&str> = pos[1].split(":").collect();
        let zone1_ood :Vec<&str> = pos[2].split(":").collect();
        let zone1_standby_ood: Vec<&str> = pos[3].split(":").collect();
        let zone1_device1: Vec<&str> = pos[4].split(":").collect();
        let zone1_device2: Vec<&str> = pos[5].split(":").collect();
    
        let zone2_people :Vec<&str> = pos[8].split(":").collect();
        let zone2_ood :Vec<&str> = pos[9].split(":").collect();
        let zone2_device1: Vec<&str> = pos[10].split(":").collect();
        let zone2_device2: Vec<&str> = pos[11].split(":").collect();
    
    
        let zone1_ood_stack = Self::open("http".to_string(), zone.zone1[0].http_port, zone.zone1[0].ws_port).await;
        let zone1_device1_stack = Self::open("http".to_string(), zone.zone1[1].http_port, zone.zone1[1].ws_port).await;
        let zone1_device2_stack = Self::open("http".to_string(), zone.zone1[2].http_port, zone.zone1[2].ws_port).await;
        let zone1_standby_ood_stack = Self::open("http".to_string(), zone.zone1[3].http_port, zone.zone1[3].ws_port).await;
    
        let zone2_ood_stack = Self::open("http".to_string(), zone.zone2[0].http_port, zone.zone2[0].ws_port).await;
        let zone2_device1_stack = Self::open("http".to_string(), zone.zone2[1].http_port, zone.zone2[1].ws_port).await;
        let zone2_device2_stack = Self::open("http".to_string(), zone.zone2[2].http_port, zone.zone2[2].ws_port).await;

        let cfg = SimZone {
            zone1_people: zone1_people[1].to_string(),
            zone1_ood_objectid: zone1_ood[1].to_string(),
            zone1_standby_ood_objectid: zone1_standby_ood[1].to_string(),
            zone1_device1_objectid: zone1_device1[1].to_string(),
            zone1_device2_objectid: zone1_device2[1].to_string(),
            zone2_people: zone2_people[1].to_string(),
            zone2_ood_objectid: zone2_ood[1].to_string(),
            zone2_device1_objectid: zone2_device1[1].to_string(),
            zone2_device2_objectid: zone2_device2[1].to_string(),

            zone1_ood_stack,
            zone1_device1_stack,
            zone1_device2_stack,
            zone1_standby_ood_stack,

            zone2_ood_stack,
            zone2_device1_stack,
            zone2_device2_stack,

        };

        return cfg;
    
    }
    
    async fn open(req_type: String, http_port: u16, ws_port: u16) -> SharedCyfsStack {
        let dec_id = ObjectId::from_str("9tGpLNnErEbyzuMgRLcRX6An1Sn8ZyimNXBdLDTgT2ze").unwrap();
        let non_http_service_url = format!("http://127.0.0.1:{}", http_port);
        let ws_url = format!("ws://127.0.0.1:{}", ws_port);
        let mut param = SharedCyfsStackParam::new_with_ws_event(Some(dec_id), &non_http_service_url, &ws_url).unwrap();
        if req_type == "ws" {
            param.requestor_config = CyfsStackRequestorConfig::ws();
        }
        let stack = SharedCyfsStack::open(param).await.unwrap();
    
        stack.wait_online(Some(std::time::Duration::from_secs(30))).await.unwrap();
    
        return stack;
    }

    pub fn get_shared_stack(&self, name: &str) -> SharedCyfsStack {
        match name {
            "zone1_device1" => return self.zone1_device1_stack.clone(),
            "zone1_device2" => return self.zone1_device2_stack.clone(),
            "zone1_ood" => return self.zone1_ood_stack.clone(),
            "zone1_standby_ood" => return self.zone1_standby_ood_stack.clone(),
            "zone2_device1" => return self.zone2_device1_stack.clone(),
            "zone2_device2" => return self.zone2_device2_stack.clone(),
            "zone2_ood" => return self.zone2_ood_stack.clone(),
            _ => unimplemented!(),
        }
    }

    pub fn get_object_id_by_name(&self, name: &str) -> ObjectId {
        match name {
            "zone1_people" => return ObjectId::from_str(self.zone1_people.as_str()).unwrap(),
            "zone1_device1" => return  ObjectId::from_str(self.zone1_device1_objectid.as_str()).unwrap(),
            "zone1_device2" => return ObjectId::from_str(self.zone1_device2_objectid.as_str()).unwrap(),
            "zone1_ood" => return ObjectId::from_str(self.zone1_ood_objectid.as_str()).unwrap(),
            "zone1_standby_ood" => return ObjectId::from_str(self.zone1_standby_ood_objectid.as_str()).unwrap(),
            "zone2_people" => return ObjectId::from_str(self.zone2_people.as_str()).unwrap(),
            "zone2_device1" => return ObjectId::from_str(self.zone2_device1_objectid.as_str()).unwrap(),
            "zone2_device2" => return ObjectId::from_str(self.zone2_device2_objectid.as_str()).unwrap(),
            "zone2_ood" => return ObjectId::from_str(self.zone2_ood_objectid.as_str()).unwrap(),
            _ => unimplemented!(),
        }
    }
    
}

