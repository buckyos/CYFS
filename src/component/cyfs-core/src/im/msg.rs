use crate::codec::*;
use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

use std::convert::TryFrom;

#[derive(Clone, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::MsgObjectContent)]
pub struct MsgObjectContent {
    pub id: ObjectId,
    pub name: String,
}

#[derive(Clone, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::MsgContent)]
pub enum MsgContent {
    Text(String),
    Object(MsgObjectContent),
}

impl MsgContent {
    fn is_text(&self) -> bool {
        match self {
            MsgContent::Text(_) => true,
            _ => false,
        }
    }
}

// MsgContent的编解码
impl ProtobufTransform<protos::MsgContent> for MsgContent {
    fn transform(value: protos::MsgContent) -> BuckyResult<Self> {
        let ret = match value.r#type {
            0 => Self::Text(value.text.unwrap()),
            1 => Self::Object(
                ProtobufTransform::transform(value.content.unwrap())?,
            ),
            _ => {
                return Err(BuckyError::new(BuckyErrorCode::Failed, format!("unknown msg content type {}", value.r#type)));
            }
        };

        Ok(ret)
    }
}

impl ProtobufTransform<&MsgContent> for protos::MsgContent {
    fn transform(value: &MsgContent) -> BuckyResult<Self> {
        let mut ret = Self::default();
        match value {
            MsgContent::Text(v) => {
                ret.r#type = 0;
                ret.text = Some(v.to_owned());
            }
            MsgContent::Object(o) => {
                ret.r#type = 1;
                ret.content = Some(ProtobufTransform::transform(o)?);
            }
        }
        Ok(ret)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::MsgDescContent)]
pub struct MsgDescContent {
    to: ObjectId,
    content: MsgContent,
}

impl DescContent for MsgDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::Msg as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type MsgType = NamedObjType<MsgDescContent, EmptyProtobufBodyContent>;
type MsgBuilder = NamedObjectBuilder<MsgDescContent, EmptyProtobufBodyContent>;

pub type MsgId = NamedObjectId<MsgType>;
pub type Msg = NamedObjectBase<MsgType>;

impl MsgDescContent {
    pub fn create(to: ObjectId, content: MsgContent) -> Self {
        Self { to, content }
    }
}

pub trait MsgObject {
    fn to(&self) -> &ObjectId;
    fn create(owner: PeopleId, to: ObjectId, content: MsgContent) -> Self;
    fn content(&self) -> &MsgContent;
    fn id(&self) -> MsgId;
    fn belongs(&self, id: &ObjectId) -> bool;
}

impl MsgObject for Msg {
    fn to(&self) -> &ObjectId {
        &self.desc().content().to
    }

    fn create(owner: PeopleId, to: ObjectId, content: MsgContent) -> Self {
        let desc_content = MsgDescContent::create(to, content);
        MsgBuilder::new(desc_content, EmptyProtobufBodyContent::default())
            .owner(owner.into())
            .build()
    }

    fn content(&self) -> &MsgContent {
        &self.desc().content().content
    }

    fn id(&self) -> MsgId {
        MsgId::try_from(self.desc().calculate_id()).unwrap()
    }

    fn belongs(&self, id: &ObjectId) -> bool {
        // 判断消息是否属于id
        if let Some(owner) = self.desc().owner().as_ref() {
            if owner == id {
                return true;
            }
        }

        self.to() == id
    }
}
