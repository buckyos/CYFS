use cyfs_base::*;

use serde_json::{Map, Value};

#[derive(Debug)]
pub struct DeviceSyncStatus {
    pub ood_device_id: DeviceId,
    pub enable_sync: bool,

    pub last_success_ping_time: u64,
    pub last_ping_result: BuckyErrorCode,
    pub last_ping_time: u64,
    pub retry_count: u32,

    pub device_root_state: ObjectId,
    pub device_root_state_revision: u64,

    pub zone_root_state: Option<ObjectId>,
    pub zone_root_state_revision: u64,
}

impl JsonCodec<DeviceSyncStatus> for DeviceSyncStatus {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();


        JsonCodecHelper::encode_string_field(
            &mut obj,
            "ood_device_id",
            &self.ood_device_id,
        );

        JsonCodecHelper::encode_bool_field(
            &mut obj,
            "enable_sync",
            self.enable_sync,
        );

        JsonCodecHelper::encode_string_field(
            &mut obj,
            "last_success_ping_time",
            &self.last_success_ping_time,
        );

        JsonCodecHelper::encode_string_field(&mut obj, "last_ping_result", &self.last_ping_result);
        JsonCodecHelper::encode_string_field(&mut obj, "last_ping_time", &self.last_ping_time);
        JsonCodecHelper::encode_string_field(&mut obj, "retry_count", &self.retry_count);

        JsonCodecHelper::encode_string_field(
            &mut obj,
            "device_root_state",
            &self.device_root_state,
        );
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "device_root_state_revision",
            &self.device_root_state_revision,
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "zone_root_state", self.zone_root_state.as_ref());
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "zone_root_state_revision",
            &self.zone_root_state_revision,
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let value: u16 = JsonCodecHelper::decode_string_field(&obj, "last_ping_result")?;
        let last_ping_result = BuckyErrorCode::from(value);

        Ok(Self {
            ood_device_id: JsonCodecHelper::decode_string_field(&obj, "ood_device_id")?,
            enable_sync: JsonCodecHelper::decode_bool_field(&obj, "enable_sync")?,

            last_success_ping_time: JsonCodecHelper::decode_string_field(
                &obj,
                "last_success_ping_time",
            )?,
            last_ping_result,
            last_ping_time: JsonCodecHelper::decode_string_field(&obj, "last_ping_time")?,
            retry_count: JsonCodecHelper::decode_string_field(&obj, "retry_count")?,

            device_root_state: JsonCodecHelper::decode_string_field(&obj, "device_root_state")?,
            device_root_state_revision: JsonCodecHelper::decode_string_field(
                &obj,
                "device_root_state_revision",
            )?,

            zone_root_state: JsonCodecHelper::decode_option_string_field(&obj, "zone_root_state")?,
            zone_root_state_revision: JsonCodecHelper::decode_string_field(
                &obj,
                "zone_root_state_revision",
            )?,
        })
    }
}
