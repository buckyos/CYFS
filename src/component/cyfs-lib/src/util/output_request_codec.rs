use super::output_request::*;
use cyfs_base::*;
use std::convert::TryFrom;
use std::path::PathBuf;

use serde_json::{Map, Value};

impl JsonCodec<UtilResolveOODOutputResponse> for UtilResolveOODOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_str_array_field(&mut obj, "device_list", &self.device_list);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            device_list: JsonCodecHelper::decode_str_array_field(obj, "device_list")?,
        })
    }
}

impl JsonCodec<DeviceStaticInfo> for DeviceStaticInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "device_id", &self.device_id);

        let buf = self.device.to_vec().unwrap();
        JsonCodecHelper::encode_string_field(&mut obj, "device", &hex::encode(&buf));

        JsonCodecHelper::encode_string_field(&mut obj, "zone_role", &self.zone_role);
        JsonCodecHelper::encode_string_field(&mut obj, "ood_work_mode", &self.ood_work_mode);

        JsonCodecHelper::encode_string_field(&mut obj, "root_state_access_mode", &self.root_state_access_mode);
        JsonCodecHelper::encode_string_field(&mut obj, "local_cache_access_mode", &self.local_cache_access_mode);

        JsonCodecHelper::encode_string_field(&mut obj, "is_ood_device", &self.is_ood_device);
        JsonCodecHelper::encode_string_field(&mut obj, "ood_device_id", &self.ood_device_id);
        JsonCodecHelper::encode_string_field(&mut obj, "zone_id", &self.zone_id);

        if let Some(id) = &self.owner_id {
            JsonCodecHelper::encode_string_field(&mut obj, "owner_id", id);
        }

        JsonCodecHelper::encode_string_field(&mut obj, "cyfs_root", &self.cyfs_root);

        if self.sn_list.len() > 0 {
            JsonCodecHelper::encode_str_array_field(&mut obj, "sn_list", &self.sn_list);
        }
        if self.known_sn_list.len() > 0 {
            JsonCodecHelper::encode_str_array_field(&mut obj, "known_sn_list", &self.known_sn_list);
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            device_id: JsonCodecHelper::decode_string_field(obj, "device_id")?,
            device: JsonCodecHelper::decode_object_field(obj, "device")?,
            zone_role: JsonCodecHelper::decode_string_field(obj, "zone_role")?,
            ood_work_mode: JsonCodecHelper::decode_string_field(obj, "ood_work_mode")?,

            root_state_access_mode: JsonCodecHelper::decode_string_field(obj, "root_state_access_mode")?,
            local_cache_access_mode: JsonCodecHelper::decode_string_field(obj, "local_cache_access_mode")?,

            is_ood_device: JsonCodecHelper::decode_string_field(obj, "is_ood_device")?,
            ood_device_id: JsonCodecHelper::decode_string_field(obj, "ood_device_id")?,
            zone_id: JsonCodecHelper::decode_string_field(obj, "zone_id")?,
            owner_id: JsonCodecHelper::decode_option_string_field(obj, "owner_id")?,
            cyfs_root: JsonCodecHelper::decode_string_field(obj, "cyfs_root")?,
            sn_list: JsonCodecHelper::decode_str_array_field(obj, "sn_list")?,
            known_sn_list: JsonCodecHelper::decode_str_array_field(obj, "known_sn_list")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<UtilGetDeviceStaticInfoOutputResponse> for UtilGetDeviceStaticInfoOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "info", &self.info);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            info: JsonCodecHelper::decode_field(obj, "info")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<BdtNetworkAccessEndpoint> for BdtNetworkAccessEndpoint {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "lan_ep", &self.lan_ep);
        JsonCodecHelper::encode_string_field(&mut obj, "wan_ep", &self.wan_ep);
        JsonCodecHelper::encode_string_field(&mut obj, "access_type", &self.access_type);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            lan_ep: JsonCodecHelper::decode_string_field(obj, "lan_ep")?,
            wan_ep: JsonCodecHelper::decode_string_field(obj, "wan_ep")?,
            access_type: JsonCodecHelper::decode_string_field(obj, "access_type")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<BdtNetworkAccessSn> for BdtNetworkAccessSn {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "sn", &self.sn);
        JsonCodecHelper::encode_string_field(&mut obj, "sn_status", &self.sn_status);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            sn: JsonCodecHelper::decode_string_field(obj, "sn")?,
            sn_status: JsonCodecHelper::decode_string_field(obj, "sn_status")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<BdtNetworkAccessInfo> for BdtNetworkAccessInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_as_list(&mut obj, "v4", &self.v4);
        JsonCodecHelper::encode_as_list(&mut obj, "v6", &self.v6);
        JsonCodecHelper::encode_as_list(&mut obj, "sn", &self.sn);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            v4: JsonCodecHelper::decode_array_field(obj, "v4")?,
            v6: JsonCodecHelper::decode_array_field(obj, "v6")?,
            sn: JsonCodecHelper::decode_array_field(obj, "sn")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<UtilGetNetworkAccessInfoOutputResponse> for UtilGetNetworkAccessInfoOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "info", &self.info);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            info: JsonCodecHelper::decode_field(obj, "info")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<VersionInfo> for VersionInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "version", &self.version);
        JsonCodecHelper::encode_string_field(&mut obj, "channel", &self.channel);
        JsonCodecHelper::encode_string_field(&mut obj, "target", &self.target);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            version: JsonCodecHelper::decode_string_field(obj, "version")?,
            channel: JsonCodecHelper::decode_string_field(obj, "channel")?,
            target: JsonCodecHelper::decode_string_field(obj, "target")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<UtilGetVersionInfoOutputResponse> for UtilGetVersionInfoOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "info", &self.info);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let ret = Self {
            info: JsonCodecHelper::decode_field(obj, "info")?,
        };

        Ok(ret)
    }
}

impl JsonCodec<UtilOutputRequestCommon> for UtilOutputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<UtilOutputRequestCommon> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<UtilBuildFileOutputRequest> for UtilBuildFileOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "local_path",
            &self.local_path.to_string_lossy().to_string(),
        );
        JsonCodecHelper::encode_string_field(&mut obj, "owner", &self.owner);
        JsonCodecHelper::encode_number_field(&mut obj, "chunk_size", self.chunk_size);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<UtilBuildFileOutputRequest> {
        let common: UtilOutputRequestCommon = JsonCodecHelper::decode_field(obj, "common")?;
        Ok(Self {
            common,
            local_path: PathBuf::from(JsonCodecHelper::decode_string_field::<String>(
                obj,
                "local_path",
            )?),
            owner: JsonCodecHelper::decode_string_field(obj, "owner")?,
            chunk_size: JsonCodecHelper::decode_int_field(obj, "chunk_size")?,
        })
    }
}

impl JsonCodec<UtilBuildFileOutputResponse> for UtilBuildFileOutputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_string_field(
            &mut obj,
            "object_raw",
            &self.object_raw.to_hex().unwrap(),
        );
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<UtilBuildFileOutputResponse> {
        Ok(Self {
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            object_raw: Vec::<u8>::clone_from_hex(
                JsonCodecHelper::decode_string_field::<String>(obj, "object_raw")?.as_str(),
                &mut Vec::new(),
            )?,
        })
    }
}

impl JsonCodec<UtilBuildDirFromObjectMapOutputRequest> for UtilBuildDirFromObjectMapOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "object_map_id", &self.object_map_id);
        JsonCodecHelper::encode_number_field(&mut obj, "dir_type", self.dir_type as u16);
        obj
    }

    fn decode_json(
        obj: &Map<String, Value>,
    ) -> BuckyResult<UtilBuildDirFromObjectMapOutputRequest> {
        let common: UtilOutputRequestCommon = JsonCodecHelper::decode_field(obj, "common")?;
        Ok(Self {
            common,
            object_map_id: JsonCodecHelper::decode_string_field(obj, "object_map_id")?,
            dir_type: BuildDirType::try_from(JsonCodecHelper::decode_int_field::<u16>(
                obj, "dir_type",
            )?)?,
        })
    }
}

impl JsonCodec<UtilBuildDirFromObjectMapOutputResponse>
    for UtilBuildDirFromObjectMapOutputResponse
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        obj
    }

    fn decode_json(
        obj: &Map<String, Value>,
    ) -> BuckyResult<UtilBuildDirFromObjectMapOutputResponse> {
        Ok(Self {
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
        })
    }
}
