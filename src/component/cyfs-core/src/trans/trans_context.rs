use std::borrow::Cow;

use crate::codec::protos::core_objects as protos;
use cyfs_base::*;
use cyfs_bdt::*;

use crate::CoreObjectType;
use serde::Serialize;

#[derive(ProtobufEncode, ProtobufDecode, ProtobufTransform, Clone, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::TransContextDescContent)]
pub struct TransContextDescContent {
    pub dec_id: ObjectId,
    pub context_path: String,
}

impl DescContent for TransContextDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::TransContext as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct TransContextDevice {
    pub target: DeviceId,
    pub chunk_codec_type: ChunkEncodeDesc,
}

#[derive(Clone, Serialize)]
pub struct TransContextBodyContent {
    pub device_list: Vec<TransContextDevice>,
}

impl BodyContent for TransContextBodyContent {
    fn version(&self) -> u8 {
        0
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl TryFrom<protos::TransContextDevice> for TransContextDevice {
    type Error = BuckyError;

    fn try_from(mut value: protos::TransContextDevice) -> BuckyResult<Self> {
        let target = cyfs_base::ProtobufCodecHelper::decode_buf(value.take_target())?;

        let info = if value.has_chunk_codec_info() {
            Some(value.take_chunk_codec_info())
        } else {
            None
        };

        let chunk_codec_type = match value.chunk_codec_type {
            protos::TransContextDevice_ChunkCodecType::Unknown => ChunkEncodeDesc::Unknown,
            _ => {
                let info = info.ok_or_else(|| {
                    let msg = format!(
                        "chunk_codec_info field missing! type={:?}",
                        value.chunk_codec_type
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                let start = if info.has_start() {
                    Some(info.get_start())
                } else {
                    None
                };
                let end = if info.has_end() {
                    Some(info.get_end())
                } else {
                    None
                };
                let step = if info.has_step() {
                    Some(info.get_step())
                } else {
                    None
                };

                match value.chunk_codec_type {
                    protos::TransContextDevice_ChunkCodecType::Stream => {
                        ChunkEncodeDesc::Stream(start, end, step)
                    }
                    protos::TransContextDevice_ChunkCodecType::Raptor => {
                        ChunkEncodeDesc::Raptor(start, end, step)
                    }
                    _ => unreachable!(),
                }
            }
        };

        Ok(Self {
            target,
            chunk_codec_type,
        })
    }
}

impl TryFrom<&TransContextDevice> for protos::TransContextDevice {
    type Error = BuckyError;

    fn try_from(value: &TransContextDevice) -> BuckyResult<Self> {
        let mut ret = protos::TransContextDevice::new();
        ret.set_target(value.target.to_vec().unwrap());

        match value.chunk_codec_type {
            ChunkEncodeDesc::Unknown => {
                ret.set_chunk_codec_type(protos::TransContextDevice_ChunkCodecType::Unknown);
            }
            _ => {
                let (start, end, step) = match value.chunk_codec_type {
                    ChunkEncodeDesc::Stream(start, end, step) => {
                        ret.set_chunk_codec_type(protos::TransContextDevice_ChunkCodecType::Stream);
                        (start, end, step)
                    }
                    ChunkEncodeDesc::Raptor(start, end, step) => {
                        ret.set_chunk_codec_type(protos::TransContextDevice_ChunkCodecType::Raptor);
                        (start, end, step)
                    }
                    _ => unreachable!(),
                };

                let mut info = protos::TransContextDeviceChunkCodecInfo::new();
                if let Some(v) = start {
                    info.set_start(v);
                }
                if let Some(v) = end {
                    info.set_end(v);
                }
                if let Some(v) = step {
                    info.set_step(v);
                }

                ret.set_chunk_codec_info(info);
            }
        }

        Ok(ret)
    }
}

impl_default_protobuf_raw_codec!(TransContextDevice);

impl TryFrom<protos::TransContextBodyContent> for TransContextBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::TransContextBodyContent) -> BuckyResult<Self> {
        let device_list =
            cyfs_base::ProtobufCodecHelper::decode_nested_list(value.take_device_list())?;
        Ok(Self { device_list })
    }
}

impl TryFrom<&TransContextBodyContent> for protos::TransContextBodyContent {
    type Error = BuckyError;

    fn try_from(value: &TransContextBodyContent) -> BuckyResult<Self> {
        let mut ret = protos::TransContextBodyContent::new();

        ret.set_device_list(cyfs_base::ProtobufCodecHelper::encode_nested_list(
            &value.device_list,
        )?);

        Ok(ret)
    }
}

impl_default_protobuf_raw_codec!(TransContextBodyContent);

pub type TransContextType = NamedObjType<TransContextDescContent, TransContextBodyContent>;
pub type TransContextBuilder = NamedObjectBuilder<TransContextDescContent, TransContextBodyContent>;
pub type TransContext = NamedObjectBase<TransContextType>;


pub struct TransContextPath;


impl TransContextPath {

    pub fn verify(path: &str) -> bool {
        if path == "/" {
            return true;
        }

        path.starts_with('/') && !path.ends_with('/')
    }

    /*
    a/b -> /a/b
    /a/b/ -> /a/b
    / -> /
    */
    pub fn fix_path(path: &str) -> Cow<str> {
        if path == "/" {
            return Cow::Borrowed(path);
        }

        let path_ret = path.trim_end_matches('/');
        if path_ret.starts_with('/') {
            Cow::Borrowed(path)
        } else {
            let path = format!("/{}", path);
            Cow::Owned(path)
        }
    }
}

pub trait TransContextObject {
    fn new(dec_id: ObjectId, context_path: &str) -> Self;
    fn gen_context_id(dec_id: ObjectId, context_path: &str) -> ObjectId;
    fn context_path(&self) -> &str;
    fn device_list(&self) -> &Vec<TransContextDevice>;
    fn device_list_mut(&mut self) -> &mut Vec<TransContextDevice>;
}

impl TransContextObject for TransContext {
    fn new(dec_id: ObjectId, context_path: &str) -> Self {
        let context_path = TransContextPath::fix_path(context_path).to_string();

        let desc = TransContextDescContent { dec_id, context_path };
        let body = TransContextBodyContent {
            device_list: vec![],
        };

        TransContextBuilder::new(desc, body)
            .no_create_time()
            .build()
    }

    fn gen_context_id(dec_id: ObjectId, context_path: &str) -> ObjectId {
        let context_path = TransContextPath::fix_path(context_path).to_string();

        let desc = TransContextDescContent { dec_id, context_path };
        NamedObjectDescBuilder::new(TransContextDescContent::obj_type(), desc)
            .option_create_time(None)
            .build()
            .calculate_id()
    }

    fn context_path(&self) -> &str {
        self.desc().content().context_path.as_str()
    }

    fn device_list(&self) -> &Vec<TransContextDevice> {
        &self.body().as_ref().unwrap().content().device_list
    }

    fn device_list_mut(&mut self) -> &mut Vec<TransContextDevice> {
        // self.body_mut().as_mut().unwrap().increase_update_time(bucky_time_now());
        &mut self.body_mut().as_mut().unwrap().content_mut().device_list
    }
}


#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;
    use cyfs_bdt::*;

    use std::str::FromStr;

    #[test]
    fn test_path() {
        let path = "/a";
        let ret = path.rsplit_once("/").unwrap();
        assert_eq!(ret.0, "");
        assert_eq!(ret.1, "a");

        let path = "/a/b";
        let ret = path.rsplit_once("/").unwrap();
        assert_eq!(ret.0, "/a");
        assert_eq!(ret.1, "b");
    }

    #[test]
    fn test() {
        let id  = ObjectId::from_str("5r4MYfFdfQ5dvAvD2WZ8wd7iKPFpWLSiAnMuTui912xL").unwrap();
        let mut context = TransContext::new(id, "/a/b/c");

        let device = TransContextDevice {
            target: DeviceId::from_str("5bnZHzXvMmqiiua3iodiaYqWR24QbZE5o8r35bH8y9Yh").unwrap(),
            chunk_codec_type: ChunkEncodeDesc::Stream(Some(1), Some(100), Some(1)),
        };
        context.device_list_mut().push(device);

        let device = TransContextDevice {
            target: DeviceId::from_str("5bnZHzXvMmqiiua3iodiaYqWR24QbZE5o8r35bH8y9Yh").unwrap(),
            chunk_codec_type: ChunkEncodeDesc::Raptor(Some(100), Some(1000), None),
        };
        context.device_list_mut().push(device);
        context.body_mut().as_mut().unwrap().increase_update_time(bucky_time_now());

        let value = context.to_vec().unwrap();
        let context2 =  TransContext::clone_from_slice(&value).unwrap();

        assert_eq!(context.device_list(), context2.device_list());
    }
}