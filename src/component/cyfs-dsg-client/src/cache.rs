// use std::{convert::TryFrom, fmt::Debug, str::FromStr};
// use cyfs_base::*;
// use cyfs_core::*;
// use cyfs_lib::*;
// use crate::{obj_id, protos};

// #[derive(Clone)]
// pub struct DsgCacheDesc {
//     pub chunks: Vec<ChunkId>, 
//     pub contracts: Vec<ObjectId> 
// }

// impl TryFrom<&DsgCacheDesc> for protos::CacheDesc {
//     type Error = BuckyError;

//     fn try_from(rust: &DsgCacheDesc) -> BuckyResult<Self> {
//         let mut proto = protos::CacheDesc::new();
//         proto.set_chunks(ProtobufCodecHelper::encode_buf_list(&rust.chunks)?);
//         proto.set_contracts(ProtobufCodecHelper::encode_buf_list(&rust.contracts)?);
//         Ok(proto)
//     }
// }

// impl TryFrom<protos::CacheDesc> for DsgCacheDesc {
//     type Error = BuckyError;

//     fn try_from(mut proto: protos::CacheDesc) -> BuckyResult<Self> {
//         Ok(Self {
//             chunks: ProtobufCodecHelper::decode_buf_list(proto.take_chunks())?, 
//             contracts: ProtobufCodecHelper::decode_buf_list(proto.take_contracts())?, 
//         })
//     }
// }


// impl DescContent for DsgCacheDesc {
//     fn obj_type() -> u16 {
//         obj_id::CACHE_OBJECT_TYPE
//     }

//     fn format(&self) -> u8 {
//         OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
//     }

//     type OwnerType = SubDescNone;
//     type AreaType = SubDescNone;
//     type AuthorType = SubDescNone;
//     type PublicKeyType = SubDescNone;
// }

// impl_default_protobuf_raw_codec!(DsgCacheDesc, protos::CacheDesc);

// #[derive(RawEncode, RawDecode, Clone)]
// pub struct DsgCacheBody {}

// impl BodyContent for DsgCacheBody {}

// pub type DsgCacheObjectType = NamedObjType<DsgCacheDesc, DsgCacheBody>;
// pub type DsgCacheObject = NamedObjectBase<DsgCacheObjectType>;

// #[derive(Copy, Clone)]
// pub struct DsgCacheObjectRef<'a> {
//     obj: &'a DsgCacheObject
// }


// impl<'a> std::fmt::Display for DsgCacheObjectRef<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "DsgCache")
//     }
// }

// impl<'a> DsgCacheObjectRef<'a> {
    
// }


