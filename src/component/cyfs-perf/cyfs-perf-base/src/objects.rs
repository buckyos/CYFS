use serde::Serialize;
use cyfs_base::*;
use crate::codec::*;

#[derive(Clone, Eq, Copy, PartialEq, Debug)]
#[repr(u16)]
pub enum PerfObjectType {
    Request = 32768,
    Accumulation = 32769,
    Action = 32770,
    Record = 32771
}

impl Into<u16> for PerfObjectType {
    fn into(self) -> u16 {
        self as u16
    }
}
impl Into<u16> for &PerfObjectType {
    fn into(self) -> u16 {
        self.clone().into()
    }
}


#[derive(Clone, Debug, Default, Serialize, ProtobufTransformType)]
#[cyfs_protobuf_type(protos::SizeResult)]
pub struct SizeResult {
    pub total: u64,
    pub avg: u64,
    pub min: u64,
    pub max: u64,
}

impl ProtobufTransform<protos::SizeResult> for SizeResult {
    fn transform(value: protos::SizeResult) -> BuckyResult<Self> {
        let total = value.total.parse::<u64>()?;
        let avg = value.avg.parse::<u64>()?;
        let min = value.min.parse::<u64>()?;
        let max = value.max.parse::<u64>()?;

        Ok(SizeResult {
            total,
            avg,
            min,
            max
        })
    }
}

impl ProtobufTransform<&SizeResult> for protos::SizeResult {
    fn transform(value: &SizeResult) -> BuckyResult<Self> {
        Ok(protos::SizeResult {
            total: value.total.to_string(),
            avg: value.avg.to_string(),
            min: value.min.to_string(),
            max: value.max.to_string()
        })
    }
}

#[derive(Clone, Debug, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::TimeResult)]
pub struct TimeResult {
    pub total: u64,
    pub avg: u64,
    pub min: u64,
    pub max: u64,
}

#[derive(Clone, Debug, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::SpeedResult)]
pub struct SpeedResult {
    pub avg: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Debug, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::PerfRequest)]
pub struct PerfRequestDesc {
    pub time: TimeResult,
    pub speed: SpeedResult,
    pub size: SizeResult,
    pub success: u32,
    pub failed: u32,
}

impl DescContent for PerfRequestDesc {
    fn obj_type() -> u16 {
        PerfObjectType::Request as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type PerfRequestType = NamedObjType<PerfRequestDesc, EmptyProtobufBodyContent>;
type PerfRequestBuilder = NamedObjectBuilder<PerfRequestDesc, EmptyProtobufBodyContent>;

pub type PerfRequestId = NamedObjectId<PerfRequestType>;
pub type PerfRequest = NamedObjectBase<PerfRequestType>;

pub trait PerfRequestObj {
    fn create(owner: ObjectId) -> PerfRequest;
    fn success(&self) -> u32;
    fn failed(&self) -> u32;
    fn add_stat(&mut self, stat: BuckyResult<Option<u64>>);
}

impl PerfRequestObj for PerfRequest {
    fn create(owner: ObjectId) -> PerfRequest {
        PerfRequestBuilder::new(PerfRequestDesc::default(), EmptyProtobufBodyContent {})
            .owner(owner)
            .build()
    }

    fn success(&self) -> u32 {
        self.desc().content().success
    }

    fn failed(&self) -> u32 {
        self.desc().content().failed
    }

    fn add_stat(&mut self, stat: BuckyResult<Option<u64>>) {
        todo!()
    }
}

#[derive(Clone, Debug, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::PerfAccumulation)]
pub struct PerfAccumulationDesc {
    pub size: SizeResult,
    pub success: u32,
    pub failed: u32,
}

impl DescContent for PerfAccumulationDesc {
    fn obj_type() -> u16 {
        PerfObjectType::Accumulation as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type PerfAccumulationType = NamedObjType<PerfAccumulationDesc, EmptyProtobufBodyContent>;
type PerfAccumulationBuilder = NamedObjectBuilder<PerfAccumulationDesc, EmptyProtobufBodyContent>;

pub type PerfAccumulationId = NamedObjectId<PerfAccumulationType>;
pub type PerfAccumulation = NamedObjectBase<PerfAccumulationType>;

pub trait PerfAccumulationObj {
    fn create(owner: ObjectId) -> PerfAccumulation;
    fn success(&self) -> u32;
    fn failed(&self) -> u32;
    fn add_stat(&mut self, stat: BuckyResult<u64>);
}

impl PerfAccumulationObj for PerfAccumulation {
    fn create(owner: ObjectId) -> PerfAccumulation {
        PerfAccumulationBuilder::new(PerfAccumulationDesc::default(), EmptyProtobufBodyContent {})
            .owner(owner)
            .build()
    }

    fn success(&self) -> u32 {
        self.desc().content().success
    }

    fn failed(&self) -> u32 {
        self.desc().content().failed
    }

    fn add_stat(&mut self, stat: BuckyResult<u64>) {
        todo!()
    }
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(protos::PerfAction)]
pub struct PerfActionDesc {
    pub err: BuckyErrorCode,
    pub key: String,
    pub value: String,
}

impl ProtobufTransform<protos::PerfAction> for PerfActionDesc {
    fn transform(value: protos::PerfAction) -> BuckyResult<Self> {
        Ok(PerfActionDesc {
            err: BuckyErrorCode::from(value.err),
            key: value.key,
            value: value.value
        })
    }
}

impl ProtobufTransform<&PerfActionDesc> for protos::PerfAction {
    fn transform(value: &PerfActionDesc) -> BuckyResult<Self> {
        Ok(protos::PerfAction {
            err: value.err.as_u16() as u32,
            key: value.key.clone(),
            value: value.value.clone()
        })
    }
}

impl DescContent for PerfActionDesc {
    fn obj_type() -> u16 {
        PerfObjectType::Accumulation as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type PerfActionType = NamedObjType<PerfActionDesc, EmptyProtobufBodyContent>;
type PerfActionBuilder = NamedObjectBuilder<PerfActionDesc, EmptyProtobufBodyContent>;

pub type PerfActionId = NamedObjectId<PerfActionType>;
pub type PerfAction = NamedObjectBase<PerfActionType>;

pub trait PerfActionObj {
    fn create(owner: ObjectId, stat: BuckyResult<(String, String)>) -> PerfAction;
    fn err_code(&self) -> BuckyErrorCode;
}

impl PerfActionObj for PerfAction {
    fn create(owner: ObjectId, stat: BuckyResult<(String, String)>) -> PerfAction {
        let (err, key, value) = match stat {
            Ok((k, v)) => {(BuckyErrorCode::Ok, k, v)}
            Err(e) => {(e.code(), "".to_owned(), "".to_owned())}
        };
        PerfActionBuilder::new(PerfActionDesc {
            err,
            key,
            value
        }, EmptyProtobufBodyContent {})
            .owner(owner)
            .build()
    }

    fn err_code(&self) -> BuckyErrorCode {
        self.desc().content().err
    }
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(protos::PerfRecord)]
pub struct PerfRecordDesc {
    pub total: u64,
    pub total_size: Option<u64>,
}

impl ProtobufTransform<protos::PerfRecord> for PerfRecordDesc {
    fn transform(value: protos::PerfRecord) -> BuckyResult<Self> {
        let total_size = if let Some(size) = value.total_size {
            Some(size.parse::<u64>()?)
        } else {
            None
        };
        Ok(PerfRecordDesc {
            total: value.total.parse::<u64>()?,
            total_size
        })
    }
}

impl ProtobufTransform<&PerfRecordDesc> for protos::PerfRecord {
    fn transform(value: &PerfRecordDesc) -> BuckyResult<Self> {
        Ok(protos::PerfRecord {
            total: value.total.to_string(),
            total_size: value.total_size.map(|f|f.to_string())
        })
    }
}

impl DescContent for PerfRecordDesc {
    fn obj_type() -> u16 {
        PerfObjectType::Accumulation as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type PerfRecordType = NamedObjType<PerfRecordDesc, EmptyProtobufBodyContent>;
type PerfRecordBuilder = NamedObjectBuilder<PerfRecordDesc, EmptyProtobufBodyContent>;

pub type PerfRecordId = NamedObjectId<PerfRecordType>;
pub type PerfRecord = NamedObjectBase<PerfRecordType>;

pub trait PerfRecordObj {
    fn create(owner: ObjectId, total: u64, total_size: Option<u64>) -> PerfRecord;
    fn total(&self) -> u64;
    fn total_size(&self) -> Option<u64>;
    fn add_stat(&mut self, total: u64, total_size: Option<u64>);
}

impl PerfRecordObj for PerfRecord {
    fn create(owner: ObjectId, total: u64, total_size: Option<u64>) -> PerfRecord {
        PerfRecordBuilder::new(PerfRecordDesc {
            total,
            total_size
        }, EmptyProtobufBodyContent {})
            .owner(owner)
            .build()
    }

    fn total(&self) -> u64 {
        self.desc().content().total
    }

    fn total_size(&self) -> Option<u64> {
        self.desc().content().total_size
    }

    fn add_stat(&mut self, total: u64, total_size: Option<u64>) {
        todo!()
    }
}

