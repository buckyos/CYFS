use crate::*;
use serde::Serialize;

/// ## 提供一个空NamedObject定义
/// 例如当一个泛型实现需要传入一个Option<N>参数，其中N是泛型NamedObject
/// 则在需要传入None时，编译器要求指定具体的N类型，此时可以使用Empty来使得调用符合None的语义

pub type EmptyType = NamedObjType<EmptyDescContent, EmptyBodyContent>;
pub type Empty = NamedObjectBase<EmptyType>;

#[derive(RawEncode, RawDecode, Clone)]
pub struct EmptyDescContent {}

impl DescContent for EmptyDescContent {
    fn obj_type() -> u16 {
        0u16
    }
    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

/* An example of custom EmptyDescContent

#[derive(Clone, Default, Serialize)]
pub struct EmptyProtobufDescContent;

impl DescContent for EmptyProtobufDescContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn obj_type() -> u16 {
        xxx
    }
    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

cyfs_base::impl_empty_protobuf_raw_codec!(EmptyProtobufDescContent);
*/


// 两个默认的空body_content
#[derive(Clone, Default, cyfs_base_derive::RawEncode, cyfs_base_derive::RawDecode, Serialize)]
pub struct EmptyBodyContent;

impl BodyContent for EmptyBodyContent {}

#[derive(Clone, Default, Serialize)]
pub struct EmptyProtobufBodyContent;

impl BodyContent for EmptyProtobufBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

crate::inner_impl_empty_protobuf_raw_codec!(EmptyProtobufBodyContent);