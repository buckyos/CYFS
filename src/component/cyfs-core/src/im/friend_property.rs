use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct FriendPropertyDescContent {}

impl DescContent for FriendPropertyDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::FriendProperty as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("FriendPropertyDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

type FriendPropertyType = NamedObjType<FriendPropertyDescContent, EmptyProtobufBodyContent>;
type FriendPropertyBuilder = NamedObjectBuilder<FriendPropertyDescContent, EmptyProtobufBodyContent>;

pub type FriendPropertyId = NamedObjectId<FriendPropertyType>;
pub type FriendProperty = NamedObjectBase<FriendPropertyType>;

//没有create time，靠签名更新事件来保持最新
pub trait FriendPropertyObject {
    fn create(owner: PeopleId) -> Self;
}

impl FriendPropertyObject for FriendProperty {
    fn create(owner: PeopleId) -> Self {
        FriendPropertyBuilder::new(
            FriendPropertyDescContent {},
            EmptyProtobufBodyContent {},
        )
            .owner(owner.into())
            .no_create_time()
            .build()
    }
}
