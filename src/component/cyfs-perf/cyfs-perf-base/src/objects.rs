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

pub(crate) trait MergeResult<T> {
    fn merge(&mut self, value: T, total_num: u32);
    fn merges(&mut self, stats: Vec<T>, total_num: u32);
}

#[derive(Clone, Debug, Default, Serialize, ProtobufTransformType)]
#[cyfs_protobuf_type(protos::SizeResult)]
pub struct SizeResult {
    pub total: u64,
    pub avg: u64,
    pub min: u64,
    pub max: u64,
}

impl MergeResult<u64> for SizeResult {
    fn merge(&mut self, value: u64, total_num: u32) {
        self.total += value;
        self.min = if self.min == 0 {value} else {self.min.min(value)};
        self.max = self.max.max(value);
        self.avg = self.total / total_num as u64;
    }

    fn merges(&mut self, stats: Vec<u64>, total_num: u32) {
        let mut min = 0;
        let mut max = 0;
        for value in stats {
            self.total += value;
            min = min.min(value);
            max = max.max(value);
        }

        self.min = if self.min == 0 {min} else {self.min.min(min)};
        self.max = self.max.max(max);
        self.avg = self.total / total_num as u64;
    }
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

impl MergeResult<u64> for TimeResult {
    fn merge(&mut self, value: u64, total_num: u32) {
        self.total += value;
        self.min = if self.min == 0 {value} else {self.min.min(value)};
        self.max = self.max.max(value);
        self.avg = self.total / total_num as u64;
    }

    fn merges(&mut self, stats: Vec<u64>, total_num: u32) {
        let mut min = 0;
        let mut max = 0;
        for value in stats {
            self.total += value;
            min = min.min(value);
            max = max.max(value);
        }

        self.min = if self.min == 0 {min} else {self.min.min(min)};
        self.max = self.max.max(max);
        self.avg = self.total / total_num as u64;
    }
}

#[derive(Clone, Debug, Default, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::SpeedResult)]
pub struct SpeedResult {
    pub avg: f32,
    pub min: f32,
    pub max: f32,
}

impl MergeResult<f32> for SpeedResult {
    fn merge(&mut self, value: f32, _total_num: u32) {
        self.min = self.min.min(value);
        self.max = self.max.max(value);

    }

    fn merges(&mut self, stats: Vec<f32>, _total_num: u32) {
        for value in stats {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
    }
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
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfRequest;
    fn success(&self) -> u32;
    fn failed(&self) -> u32;
    fn add_stat(&self, stat: &PerfRequestItem) -> PerfRequest;
    fn add_stats(&self, stats: &[PerfRequestItem]) -> PerfRequest;
}

#[derive(Clone)]
pub struct PerfRequestItem {
    pub time: u64,
    pub spend_time: u64,
    pub err: BuckyErrorCode,
    pub stat: Option<u64>
}

impl PerfRequestObj for PerfRequest {
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfRequest {
        PerfRequestBuilder::new(PerfRequestDesc::default(), EmptyProtobufBodyContent {})
            .owner(owner)
            .dec_id(dec_id)
            .build()
    }

    fn success(&self) -> u32 {
        self.desc().content().success
    }

    fn failed(&self) -> u32 {
        self.desc().content().failed
    }

    // spend_time: ms
    fn add_stat(&self, stat: &PerfRequestItem) -> PerfRequest {
        let mut desc = self.desc().content().clone();
        if stat.err == BuckyErrorCode::Ok {
            desc.success += 1;
            desc.time.merge(stat.spend_time, desc.success);

            if let Some(stat_num) = stat.stat {
                desc.size.merge(stat_num, desc.success);

                let speed = (stat_num / stat.spend_time / 1000) as f32;
                desc.speed.avg = (desc.size.total  / desc.time.total / 1000) as f32;
                desc.speed.merge(speed, 0);
            }
        } else {
            desc.failed += 1;
        }

        PerfRequestBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }

    fn add_stats(&self, stats: &[PerfRequestItem]) -> PerfRequest {
        let mut desc = self.desc().content().clone();

        let mut spend_times = vec![];
        let mut stat_nums = vec![];
        let mut speeds = vec![];
        for item in stats {
            if item.err == BuckyErrorCode::Ok {
                desc.success += 1;
                spend_times.push(item.spend_time);

                if let Some(stat) = item.stat {
                    stat_nums.push(stat);
                    speeds.push((stat / item.spend_time / 1000) as f32)
                }

            } else {
                desc.failed += 1;
            }
        };

        desc.time.merges(spend_times, desc.success);
        desc.size.merges(stat_nums, desc.success);

        desc.speed.avg = (desc.size.total  / desc.time.total / 1000) as f32;
        desc.speed.merges(speeds, 0);

        PerfRequestBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }
}

#[derive(Clone)]
pub struct PerfAccumulationItem {
    pub time: u64,
    pub err: BuckyErrorCode,
    pub stat: u64
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
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfAccumulation;
    fn success(&self) -> u32;
    fn failed(&self) -> u32;
    fn add_stat(&self, stat: &PerfAccumulationItem) -> PerfAccumulation;
    fn add_stats(&self, stats: &[PerfAccumulationItem]) -> PerfAccumulation;
}

impl PerfAccumulationObj for PerfAccumulation {
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfAccumulation {
        PerfAccumulationBuilder::new(PerfAccumulationDesc::default(), EmptyProtobufBodyContent {})
            .owner(owner)
            .dec_id(dec_id)
            .build()
    }

    fn success(&self) -> u32 {
        self.desc().content().success
    }

    fn failed(&self) -> u32 {
        self.desc().content().failed
    }

    fn add_stat(&self, stat: &PerfAccumulationItem) -> PerfAccumulation {
        let mut desc = self.desc().content().clone();
        if stat.err == BuckyErrorCode::Ok {
            desc.success += 1;
            desc.size.merge(stat.stat, desc.success);
        } else {
            desc.failed += 1;
        }

        PerfAccumulationBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }

    fn add_stats(&self, stats: &[PerfAccumulationItem]) -> PerfAccumulation {
        let mut desc = self.desc().content().clone();
        let mut stat_nums = vec![];
        for stat in stats {
            if stat.err == BuckyErrorCode::Ok {
                desc.success += 1;
                stat_nums.push(stat.stat);
            } else {
                desc.failed += 1;
            }
        }

        desc.size.merges(stat_nums, desc.success);

        PerfAccumulationBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(protos::PerfActionItem)]
pub struct PerfActionItem {
    pub err: BuckyErrorCode,
    pub time: u64,
    pub key: String,
    pub value: String
}

impl ProtobufTransform<protos::PerfActionItem> for PerfActionItem {
    fn transform(value: protos::PerfActionItem) -> BuckyResult<Self> {
        Ok(PerfActionItem {
            err: BuckyErrorCode::from(value.err),
            time: value.time.parse::<u64>()?,
            key: value.key,
            value: value.value
        })
    }
}

impl ProtobufTransform<&PerfActionItem> for protos::PerfActionItem {
    fn transform(value: &PerfActionItem) -> BuckyResult<Self> {
        Ok(protos::PerfActionItem {
            err: value.err.as_u16() as u32,
            time: value.time.to_string(),
            key: value.key.clone(),
            value: value.value.clone()
        })
    }
}

impl PerfActionItem {
    pub(crate) fn create(err: BuckyErrorCode, key: String, value: String) -> PerfActionItem {
        return Self {
            err,
            time: bucky_time_now(),
            key,
            value
        }
    }
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(protos::PerfAction)]
pub struct PerfActionDesc {
    pub actions: Vec<PerfActionItem>
}

impl DescContent for PerfActionDesc {
    fn obj_type() -> u16 {
        PerfObjectType::Action as u16
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
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfAction;
    fn add_stat(&self, stat: PerfActionItem) -> PerfAction;
    fn add_stats(&self, stat: &mut Vec<PerfActionItem>) -> PerfAction;
}

impl PerfActionObj for PerfAction {
    fn create(owner: ObjectId, dec_id: ObjectId) -> PerfAction {
        PerfActionBuilder::new(PerfActionDesc {
            actions: vec![],
        }, EmptyProtobufBodyContent {})
            .owner(owner)
            .dec_id(dec_id)
            .build()
    }

    fn add_stat(&self, stat: PerfActionItem) -> PerfAction {
        let mut desc = self.desc().content().clone();
        desc.actions.push(stat);

        PerfActionBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }

    fn add_stats(&self, stat: &mut Vec<PerfActionItem>) -> PerfAction {
        let mut desc = self.desc().content().clone();
        desc.actions.append(stat);

        PerfActionBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
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
        PerfObjectType::Record as u16
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
    fn create(owner: ObjectId, dec_id: ObjectId, total: u64, total_size: Option<u64>) -> PerfRecord;
    fn total(&self) -> u64;
    fn total_size(&self) -> Option<u64>;
    fn add_stat(&self, total: u64, total_size: Option<u64>) -> PerfRecord;
}

impl PerfRecordObj for PerfRecord {
    fn create(owner: ObjectId, dec_id: ObjectId, total: u64, total_size: Option<u64>) -> PerfRecord {
        PerfRecordBuilder::new(PerfRecordDesc {
            total,
            total_size
        }, EmptyProtobufBodyContent {})
            .owner(owner)
            .dec_id(dec_id)
            .build()
    }

    fn total(&self) -> u64 {
        self.desc().content().total
    }

    fn total_size(&self) -> Option<u64> {
        self.desc().content().total_size
    }

    fn add_stat(&self, total: u64, total_size: Option<u64>) -> PerfRecord {
        let mut desc = self.desc().content().clone();
        desc.total = total;
        desc.total_size = total_size;

        PerfRecordBuilder::new(desc, EmptyProtobufBodyContent {})
            .owner(self.desc().owner().unwrap())
            .dec_id(self.desc().dec_id().unwrap())
            .build()
    }
}

