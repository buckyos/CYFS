use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::FriendOptionContent)]
pub struct FriendOptionDescContent {
    auto_confirm: Option<u8>,
    msg: Option<String>,
}

impl DescContent for FriendOptionDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::FriendOption as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("FriendOptionDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

type FriendOptionType = NamedObjType<FriendOptionDescContent, EmptyProtobufBodyContent>;
type FriendOptionBuilder = NamedObjectBuilder<FriendOptionDescContent, EmptyProtobufBodyContent>;

pub type FriendOptionId = NamedObjectId<FriendOptionType>;
pub type FriendOption = NamedObjectBase<FriendOptionType>;

//没有create time，靠签名更新事件来保持最新
pub trait FriendOptionObject {
    fn create(owner: PeopleId, auto_confirm: Option<bool>, msg: Option<String>) -> Self;
    fn auto_confirm(&self) -> Option<bool>;
    fn msg(&self) -> Option<&str>;
}

impl FriendOptionObject for FriendOption {
    fn create(owner: PeopleId, auto_confirm: Option<bool>, msg: Option<String>) -> Self {
        FriendOptionBuilder::new(
            FriendOptionDescContent {
                auto_confirm: auto_confirm.map(|b| if b { 1 } else { 0 }),
                msg,
            },
            EmptyProtobufBodyContent {},
        )
        .owner(owner.into())
        .no_create_time()
        .build()
    }

    fn auto_confirm(&self) -> Option<bool> {
        self.desc().content().auto_confirm.map(|v| v == 1)
    }

    fn msg(&self) -> Option<&str> {
        self.desc()
            .content()
            .msg
            .as_ref()
            .map(|s| s.as_str())
    }
}
