use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct FriendOptionDescContent {}

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

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::FriendOptionContent)]
pub struct FriendOptionContent {
    auto_confirm: Option<u8>,
    msg: Option<String>,
}

impl BodyContent for FriendOptionContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type FriendOptionType = NamedObjType<FriendOptionDescContent, FriendOptionContent>;
type FriendOptionBuilder = NamedObjectBuilder<FriendOptionDescContent, FriendOptionContent>;

pub type FriendOptionId = NamedObjectId<FriendOptionType>;
pub type FriendOption = NamedObjectBase<FriendOptionType>;

impl FriendOptionDescContent {
    pub fn new() -> Self {
        Self {}
    }
}

//没有create time，靠签名更新事件来保持最新
pub trait FriendOptionObject {
    fn create(owner: PeopleId, auto_confirm: Option<bool>, msg: Option<String>) -> Self;
    fn auto_confirm(&self) -> Option<bool>;
    fn msg(&self) -> Option<&str>;
}

impl FriendOptionObject for FriendOption {
    fn create(owner: PeopleId, auto_confirm: Option<bool>, msg: Option<String>) -> Self {
        let desc = FriendOptionDescContent::new();

        FriendOptionBuilder::new(
            desc,
            FriendOptionContent {
                auto_confirm: auto_confirm.map(|b| if b { 1 } else { 0 }),
                msg,
            },
        )
        .owner(owner.into())
        .no_create_time()
        .build()
    }

    fn auto_confirm(&self) -> Option<bool> {
        self.body_expect("").content().auto_confirm.map(|v| v == 1)
    }

    fn msg(&self) -> Option<&str> {
        self.body_expect("")
            .content()
            .msg
            .as_ref()
            .map(|s| s.as_str())
    }
}
