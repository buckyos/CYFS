use crate::codec::*;
use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

use std::convert::TryInto;
use std::str::FromStr;

#[derive(Debug, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::ZoneDescContent)]
pub struct ZoneDescContent {
    // People/SimpleGroup
    // 一个device在没有owner情况下，所在zone的owner就是自己，并且该zone只有自己一台设备
    owner: ObjectId,
}

impl DescContent for ZoneDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::Zone as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::ZoneBodyContent)]
pub struct ZoneBodyContent {
    ood_work_mode: OODWorkMode,
    ood_list: Vec<DeviceId>,
    known_device_list: Vec<DeviceId>,
}

impl BodyContent for ZoneBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ProtobufTransform<protos::ZoneBodyContent> for ZoneBodyContent {
    fn transform(value: protos::ZoneBodyContent) -> BuckyResult<Self> {
        let mut ret = Self {
            ood_list: ProtobufCodecHelper::decode_buf_list(value.ood_list)?,
            known_device_list: ProtobufCodecHelper::decode_buf_list(value.known_device_list)?,
            ood_work_mode: OODWorkMode::Standalone,
        };

        if let Some(ood_work_mode) = value.ood_work_mode {
            ret.ood_work_mode = OODWorkMode::from_str(&ood_work_mode)?;
        }

        Ok(ret)
    }
}
impl ProtobufTransform<&ZoneBodyContent> for protos::ZoneBodyContent {
    fn transform(value: &ZoneBodyContent) -> BuckyResult<Self> {
        let ret = Self {
            ood_list: ProtobufCodecHelper::encode_buf_list(&value.ood_list)?.into_vec(),
            known_device_list: ProtobufCodecHelper::encode_buf_list(&value.known_device_list)?
                .into_vec(),
            ood_work_mode: Some(value.ood_work_mode.to_string()),
        };

        Ok(ret)
    }
}

type ZoneType = NamedObjType<ZoneDescContent, ZoneBodyContent>;
type ZoneBuilder = NamedObjectBuilder<ZoneDescContent, ZoneBodyContent>;
type ZoneDesc = NamedObjectDesc<ZoneDescContent>;

pub type ZoneId = NamedObjectId<ZoneType>;
pub type Zone = NamedObjectBase<ZoneType>;

pub trait ZoneObj {
    fn create(
        owner: ObjectId,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        known_device_list: Vec<DeviceId>,
    ) -> Self;
    fn owner(&self) -> &ObjectId;

    fn ood_work_mode(&self) -> &OODWorkMode;
    fn set_ood_work_mode(&mut self, work_mode: OODWorkMode);

    fn ood(&self) -> &DeviceId;
    fn ood_list(&self) -> &Vec<DeviceId>;
    fn ood_list_mut(&mut self) -> &mut Vec<DeviceId>;

    fn ood_index(&self, device_id: &DeviceId) -> BuckyResult<usize>;

    fn known_device_list(&self) -> &Vec<DeviceId>;
    fn known_device_list_mut(&mut self) -> &mut Vec<DeviceId>;

    fn device_index(&self, device_id: &DeviceId) -> BuckyResult<usize>;

    fn zone_id(&self) -> ZoneId;
    fn is_ood(&self, device_id: &DeviceId) -> bool;
    fn is_known_device(&self, device_id: &DeviceId) -> bool;
}

impl ZoneObj for Zone {
    fn create(
        owner: ObjectId,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        known_device_list: Vec<DeviceId>,
    ) -> Self {
        assert!(ood_list.len() > 0);

        let body = ZoneBodyContent {
            ood_work_mode,
            ood_list,
            known_device_list,
        };
        let desc = ZoneDescContent { owner };
        ZoneBuilder::new(desc, body).no_create_time().build()
    }

    fn owner(&self) -> &ObjectId {
        &self.desc().content().owner
    }

    fn ood_work_mode(&self) -> &OODWorkMode {
        &self.body().as_ref().unwrap().content().ood_work_mode
    }

    fn set_ood_work_mode(&mut self, work_mode: OODWorkMode) {
        self.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .ood_work_mode = work_mode;
    }

    fn ood(&self) -> &DeviceId {
        &self.body().as_ref().unwrap().content().ood_list[0]
    }

    fn ood_list(&self) -> &Vec<DeviceId> {
        &self.body().as_ref().unwrap().content().ood_list
    }

    fn ood_list_mut(&mut self) -> &mut Vec<DeviceId> {
        &mut self.body_mut().as_mut().unwrap().content_mut().ood_list
    }

    fn ood_index(&self, device_id: &DeviceId) -> BuckyResult<usize> {
        for (i, id) in self
            .body()
            .as_ref()
            .unwrap()
            .content()
            .ood_list
            .iter()
            .enumerate()
        {
            if id == device_id {
                return Ok(i);
            }
        }

        Err(BuckyError::from(BuckyErrorCode::NotFound))
    }

    fn known_device_list(&self) -> &Vec<DeviceId> {
        &self.body().as_ref().unwrap().content().known_device_list
    }

    fn known_device_list_mut(&mut self) -> &mut Vec<DeviceId> {
        &mut self
            .body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .known_device_list
    }

    fn device_index(&self, device_id: &DeviceId) -> BuckyResult<usize> {
        for (i, id) in self
            .body()
            .as_ref()
            .unwrap()
            .content()
            .known_device_list
            .iter()
            .enumerate()
        {
            if id == device_id {
                return Ok(i);
            }
        }

        Err(BuckyError::from(BuckyErrorCode::NotFound))
    }

    fn zone_id(&self) -> ZoneId {
        self.desc().calculate_id().try_into().unwrap()
    }

    fn is_known_device(&self, device_id: &DeviceId) -> bool {
        if self.is_ood(device_id) {
            return true;
        }

        self.body()
            .as_ref()
            .unwrap()
            .content()
            .known_device_list
            .iter()
            .any(|id| id == device_id)
    }

    fn is_ood(&self, device_id: &DeviceId) -> bool {
        self.ood_list().iter().find(|&v| *v == *device_id).is_some()
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;

    use std::str::FromStr;

    #[test]
    fn test() {
        let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();
        let ood = DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();

        let zone = Zone::create(
            owner,
            OODWorkMode::Standalone,
            vec![ood.clone()],
            vec![ood.clone()],
        );
        let buf = zone.to_vec().unwrap();
        let zone2 = Zone::clone_from_slice(&buf).unwrap();

        assert_eq!(zone2.owner(), zone.owner());
        assert_eq!(zone2.ood_list()[0], ood);
        assert_eq!(zone2.known_device_list()[0], ood);

        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("zone.desc");
        std::fs::write(&name, buf).unwrap();
    }
}
