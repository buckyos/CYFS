use crate::codec::*;
use crate::items::*;
use cyfs_base::*;
use cyfs_core::CoreObjectType;

use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

#[derive(Clone)]
pub struct PerfDescContent {
    device: DeviceId,

    // zone的owner，支持people和simplegroup等有权对象
    people: ObjectId,

    // dec id
    id: String,

    // dec client版本信息
    version: String,

    // 记录body的hash，用以区分对象
    hash: String,
}

impl DescContent for PerfDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::PerfOperation as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("PerfDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

// PerfDescContent基于protobuf编解码
impl TryFrom<protos::PerfDescContent> for PerfDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfDescContent) -> BuckyResult<Self> {
        Ok(Self {
            device: ProtobufCodecHelper::decode_buf(value.take_device())?,
            people: ProtobufCodecHelper::decode_buf(value.take_people())?,
            id: value.take_id(),
            version: value.take_version(),
            hash: value.take_hash(),
        })
    }
}
impl TryFrom<&PerfDescContent> for protos::PerfDescContent {
    type Error = BuckyError;

    fn try_from(value: &PerfDescContent) -> BuckyResult<Self> {
        let mut ret = Self::new();
        ret.set_device(value.device.to_vec()?);
        ret.set_people(value.people.to_vec()?);
        ret.set_id(value.id.clone());
        ret.set_version(value.version.clone());
        ret.set_hash(value.hash.clone());

        Ok(ret)
    }
}

::cyfs_base::impl_default_protobuf_raw_codec!(PerfDescContent);

#[derive(Clone)]
pub struct PerfBodyContent {
    time_range: PerfTimeRange,
    all: HashMap<String, PerfIsolateEntity>,
}

impl BodyContent for PerfBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl PerfBodyContent {
    pub fn new(list: PerfIsolateEntityList) -> Self {
        Self {
            time_range: list.time_range,
            all: list.list,
        }
    }

    pub fn mut_all(&mut self) -> &mut HashMap<String, PerfIsolateEntity> {
        &mut self.all
    }
}

impl TryFrom<protos::PerfBodyContent> for PerfBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::PerfBodyContent) -> BuckyResult<Self> {
        let time_range = PerfTimeRange::try_from(value.take_time_range())?;

        let mut all = HashMap::new();
        for (k, v) in value.take_all() {
            all.insert(k, ProtobufCodecHelper::decode_nested_item(v)?);
        }

        let ret = PerfBodyContent { time_range, all };

        Ok(ret)
    }
}
impl TryFrom<&PerfBodyContent> for protos::PerfBodyContent {
    type Error = BuckyError;

    fn try_from(value: &PerfBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        let time_range = (&value.time_range).try_into().unwrap();
        ret.set_time_range(time_range);

        let mut all = HashMap::new();
        for (k, v) in &value.all {
            all.insert(k.to_owned(), ProtobufCodecHelper::encode_nested_item(v)?);
        }

        ret.set_all(all);

        Ok(ret)
    }
}

::cyfs_base::impl_default_protobuf_raw_codec!(PerfBodyContent);

type PerfType = NamedObjType<PerfDescContent, PerfBodyContent>;
type PerfBuilder = NamedObjectBuilder<PerfDescContent, PerfBodyContent>;
type PerfDesc = NamedObjectDesc<PerfDescContent>;

pub type PerfId = NamedObjectId<PerfType>;
pub type Perf = NamedObjectBase<PerfType>;

impl PerfDescContent {
    pub fn new(
        device: DeviceId,
        people: ObjectId,
        id: String,
        version: String,
        hash: String,
    ) -> Self {
        Self {
            device,
            people,
            id,
            version,
            hash,
        }
    }
}

pub trait PerfObject {
    fn create(
        device: DeviceId,
        people: ObjectId,
        dec_id: Option<ObjectId>,
        id: String,
        version: String,
        list: PerfIsolateEntityList,
    ) -> Self;
    fn device(&self) -> String;
    fn people(&self) -> String;
    fn get_id(&self) -> &String;
    fn get_version(&self) -> &String;
    fn get_hash(&self) -> &String;

    fn get_time_range(&self) -> &PerfTimeRange;
    fn get_entity_list(&self) -> &HashMap<String, PerfIsolateEntity>;

    fn perf_id(&self) -> PerfId;

    fn dec_id(&self) -> ObjectId;
}

impl PerfObject for Perf {
    fn create(
        device: DeviceId,
        people: ObjectId,
        dec_id: Option<ObjectId>,
        id: String,
        version: String,
        list: PerfIsolateEntityList,
    ) -> Self {
        let body = PerfBodyContent {
            time_range: list.time_range,
            all: list.list,
        };

        // 计算body的hash，用以唯一表示这个对象
        let buf = body.to_vec().unwrap();
        let hash = hash_data(&buf).to_string();

        let desc = PerfDescContent::new(device.clone(), people, id, version, hash);

        // 使用device作为owner，并使用调用者所在dec_id
        PerfBuilder::new(desc, body)
            .owner(device.into())
            .option_dec_id(dec_id)
            .no_create_time()
            .build()
    }

    fn device(&self) -> String {
        self.desc().content().device.to_string()
    }

    fn people(&self) -> String {
        self.desc().content().people.to_string()
    }

    fn get_id(&self) -> &String {
        &self.desc().content().id
    }
    fn get_version(&self) -> &String {
        &self.desc().content().version
    }
    fn get_hash(&self) -> &String {
        &self.desc().content().hash
    }

    fn get_time_range(&self) -> &PerfTimeRange {
        &self.body_expect("").content().time_range
    }

    fn get_entity_list(&self) -> &HashMap<String, PerfIsolateEntity> {
        &self.body_expect("").content().all
    }

    fn perf_id(&self) -> PerfId {
        self.desc().calculate_id().try_into().unwrap()
    }

    fn dec_id(&self) -> ObjectId {
        self.desc().dec_id().unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;

    #[test]
    fn test_empty() {
        let owner = ObjectId::default();
        let device = DeviceId::default();
        let app_dec_id: ObjectId = Default::default();
        let list = PerfIsolateEntityList::default();
        let perf_obj = Perf::create(device, owner, Some(app_dec_id), "".to_owned(), "".to_owned(), list);

        let buf = perf_obj.to_vec().unwrap();

        println!("empty perf_id: {}", perf_obj.perf_id());
        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("perf_empty.desc");
        std::fs::write(&name, buf).unwrap();
    }

    #[test]
    fn test_codec() {
        let owner = ObjectId::default();
        let device = DeviceId::default();
        let app_dec_id: ObjectId = Default::default();

        let list = PerfIsolateEntityList::default();
        let perf_obj = Perf::create(device, owner, Some(app_dec_id), "test".to_owned(), "1.0.0".to_owned(), list);

        let perf_id = perf_obj.desc().calculate_id();
        let buf = perf_obj.to_vec().unwrap();

        let perf_obj2 = Perf::clone_from_slice(&buf).unwrap();
        assert_eq!(perf_id, perf_obj2.desc().calculate_id());

        let (any, left_buf) = AnyNamedObject::raw_decode(&buf).unwrap();
        assert_eq!(left_buf.len(), 0);
        log::info!("any id={}", any.calculate_id());
        assert_eq!(perf_id, any.calculate_id());

        let buf2 = any.to_vec().unwrap();
        assert_eq!(buf.len(), buf2.len());
        assert_eq!(buf, buf2);

        // 保存到文件
        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("perf.desc");
        std::fs::write(&name, buf2).unwrap();
    }
}
