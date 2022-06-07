use crate::*;

use std::convert::TryFrom;
use std::str::FromStr;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SimpleGroupDescContent {}

impl DescContent for SimpleGroupDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::SimpleGroup.into()
    }

    type OwnerType = SubDescNone;
    type AreaType = Option<Area>;
    type AuthorType = SubDescNone;
    type PublicKeyType = MNPublicKey;
}

#[derive(Clone, Debug)]
pub struct SimpleGroupBodyContent {
    members: Vec<ObjectId>,
    ood_list: Vec<DeviceId>,
    ood_work_mode: Option<OODWorkMode>,
}

impl BodyContent for SimpleGroupBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl SimpleGroupBodyContent {
    pub fn new(
        members: Vec<ObjectId>,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            members,
            ood_work_mode: Some(ood_work_mode),
            ood_list,
        }
    }

    pub fn members(&self) -> &Vec<ObjectId> {
        &self.members
    }

    pub fn members_mut(&mut self) -> &mut Vec<ObjectId> {
        &mut self.members
    }

    pub fn ood_list(&self) -> &Vec<DeviceId> {
        &self.ood_list
    }

    pub fn ood_list_mut(&mut self) -> &mut Vec<DeviceId> {
        &mut self.ood_list
    }

    pub fn ood_work_mode(&self) -> OODWorkMode {
        self.ood_work_mode
            .clone()
            .unwrap_or(OODWorkMode::Standalone)
    }

    pub fn set_ood_work_mode(&mut self, ood_work_mode: OODWorkMode) {
        self.ood_work_mode = Some(ood_work_mode);
    }
}

// body使用protobuf编解码
impl TryFrom<protos::SimpleGroupBodyContent> for SimpleGroupBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::SimpleGroupBodyContent) -> BuckyResult<Self> {
        let mut ret = Self {
            members: ProtobufCodecHelper::decode_buf_list(value.take_members())?,
            ood_list: ProtobufCodecHelper::decode_buf_list(value.take_ood_list())?,
            ood_work_mode: None,
        };

        if value.has_ood_work_mode() {
            ret.ood_work_mode = Some(OODWorkMode::from_str(value.get_ood_work_mode())?);
        }

        Ok(ret)
    }
}

impl TryFrom<&SimpleGroupBodyContent> for protos::SimpleGroupBodyContent {
    type Error = BuckyError;

    fn try_from(value: &SimpleGroupBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_members(ProtobufCodecHelper::encode_buf_list(&value.members)?);
        ret.set_ood_list(ProtobufCodecHelper::encode_buf_list(&value.ood_list)?);

        if let Some(ood_work_mode) = &value.ood_work_mode {
            ret.set_ood_work_mode(ood_work_mode.to_string());
        }

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(SimpleGroupBodyContent);

pub type SimpleGroupType = NamedObjType<SimpleGroupDescContent, SimpleGroupBodyContent>;
pub type SimpleGroupBuilder = NamedObjectBuilder<SimpleGroupDescContent, SimpleGroupBodyContent>;

pub type SimpleGroupDesc = NamedObjectDesc<SimpleGroupDescContent>;
pub type SimpleGroupId = NamedObjectId<SimpleGroupType>;
pub type SimpleGroup = NamedObjectBase<SimpleGroupType>;

impl SimpleGroupDesc {
    pub fn simple_group_id(&self) -> SimpleGroupId {
        SimpleGroupId::try_from(self.calculate_id()).unwrap()
    }
}

impl SimpleGroup {
    pub fn new(
        threshold: u8,
        owners: Vec<PublicKey>,
        members: Vec<ObjectId>,
        ood_work_mode: OODWorkMode,
        ood_list: Vec<DeviceId>,
        area: Area,
    ) -> SimpleGroupBuilder {
        let desc_content = SimpleGroupDescContent {};

        let body_content = SimpleGroupBodyContent::new(members, ood_work_mode, ood_list);

        SimpleGroupBuilder::new(desc_content, body_content)
            .area(area)
            .public_key((threshold, owners))
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn simple_group() {
        let threshold = 0;

        let members = vec![ObjectId::default()];

        let ood_list = vec![DeviceId::default()];

        let obj = SimpleGroup::new(
            threshold,
            vec![],
            members,
            OODWorkMode::Standalone,
            ood_list,
            Area::default(),
        )
        .build();
        // let p = Path::new("f:\\temp\\simple_group.obj");
        // if p.parent().unwrap().exists() {
        //     obj.clone().encode_to_file(p, false);
        // }

        let buf = obj.to_vec().unwrap();

        let decode_obj = SimpleGroup::clone_from_slice(&buf).unwrap();

        assert!(obj.desc().simple_group_id() == decode_obj.desc().simple_group_id());
    }
}
