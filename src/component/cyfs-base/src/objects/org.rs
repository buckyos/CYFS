use crate::*;

use std::convert::TryFrom;
use serde::Serialize;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct OrgDescContent {}

impl DescContent for OrgDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Org.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, Serialize)]
pub struct Director {
    pub id: ObjectId,
    pub right: u8,
}

pub struct BoardOfDirector {}

pub enum DepartmentMember {}

pub struct Department {}

#[derive(Clone, Debug, Serialize)]
pub struct OrgMember {
    pub id: ObjectId,
    pub right: u8,
    pub shares: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrgBodyContent {
    pub members: Vec<OrgMember>,
    pub directors: Vec<Director>,
    pub total_equity: u64,
}

impl BodyContent for OrgBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

// body使用protobuf编解码
impl TryFrom<protos::Director> for Director {
    type Error = BuckyError;

    fn try_from(mut value: protos::Director) -> BuckyResult<Self> {
        Ok(Self {
            id: ProtobufCodecHelper::decode_buf(value.take_id())?,
            right: value.get_right() as u8,
        })
    }
}

impl TryFrom<&Director> for protos::Director {
    type Error = BuckyError;

    fn try_from(value: &Director) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_id(value.id.to_vec()?);
        ret.set_right(value.right as u32);

        Ok(ret)
    }
}

impl TryFrom<protos::OrgMember> for OrgMember {
    type Error = BuckyError;

    fn try_from(mut value: protos::OrgMember) -> BuckyResult<Self> {
        Ok(Self {
            id: ProtobufCodecHelper::decode_buf(value.take_id())?,
            right: value.get_right() as u8,
            shares: value.get_shares(),
        })
    }
}

impl TryFrom<&OrgMember> for protos::OrgMember {
    type Error = BuckyError;

    fn try_from(value: &OrgMember) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_id(value.id.to_vec()?);
        ret.set_right(value.right as u32);
        ret.set_shares(value.shares);

        Ok(ret)
    }
}

impl TryFrom<protos::OrgBodyContent> for OrgBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::OrgBodyContent) -> BuckyResult<Self> {
        Ok(Self {
            members: ProtobufCodecHelper::decode_nested_list(value.take_members())?,
            directors: ProtobufCodecHelper::decode_nested_list(value.take_directors())?,
            total_equity: value.total_equity,
        })
    }
}

impl TryFrom<&OrgBodyContent> for protos::OrgBodyContent {
    type Error = BuckyError;

    fn try_from(value: &OrgBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_members(ProtobufCodecHelper::encode_nested_list(&value.members)?);
        ret.set_directors(ProtobufCodecHelper::encode_nested_list(&value.directors)?);
        ret.set_total_equity(value.total_equity);

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(OrgBodyContent);

pub type OrgType = NamedObjType<OrgDescContent, OrgBodyContent>;
pub type OrgBuilder = NamedObjectBuilder<OrgDescContent, OrgBodyContent>;

pub type OrgDesc = NamedObjectDesc<OrgDescContent>;
pub type OrgId = NamedObjectId<OrgType>;
pub type Org = NamedObjectBase<OrgType>;

impl OrgDesc {
    pub fn action_id(&self) -> OrgId {
        OrgId::try_from(self.calculate_id()).unwrap()
    }
}

impl NamedObjectBase<OrgType> {
    pub fn new(members: Vec<OrgMember>, directors: Vec<Director>) -> OrgBuilder {
        let desc_content = OrgDescContent {};
        let body_content = OrgBodyContent {
            members,
            directors,
            total_equity: 0,
        };
        OrgBuilder::new(desc_content, body_content)
    }

    pub fn members(&self) -> &Vec<OrgMember> {
        &self.body().as_ref().unwrap().content().members
    }

    pub fn members_mut(&mut self) -> &mut Vec<OrgMember> {
        &mut self.body_mut().as_mut().unwrap().content_mut().members
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    //use std::path::Path;

    #[test]
    fn org() {
        let action = Org::new(vec![], vec![]).build();

        // let p = Path::new("f:\\temp\\org.obj");
        // if p.parent().unwrap().exists() {
        //     action.clone().encode_to_file(p, false);
        // }

        let buf = action.to_vec().unwrap();
        let _obj = Org::clone_from_slice(&buf).unwrap();
    }
}
