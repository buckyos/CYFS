use crate::*;

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
