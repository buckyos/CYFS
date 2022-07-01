use crate::*;

use serde::Serialize;
use serde_json::{Map, Value};

pub trait ObjectFormat {
    fn format_json(&self) -> serde_json::Value;
}

// auto impl ObjectFormat for struct which use Serialize macros + ObjectFormatAutoWithSerde
impl<T> ObjectFormat for T
where
    T: Serialize + ObjectFormatAutoWithSerde,
{
    fn format_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

pub trait ObjectFormatAutoWithSerde {}

#[macro_export]
macro_rules! object_format_empty_impl {
    ($content:ty) => {
        impl ObjectFormat for $content {
            fn format_json(&self) -> serde_json::Value {
                serde_json::Map::new().into()
            }
        }
    };
}

#[macro_export]
macro_rules! object_format_not_impl {
    ($content:ty) => {
        impl ObjectFormat for $content {
            fn format_json(&self) -> serde_json::Value {
                serde_json::Value::String("[unimplemented]".to_owned())
            }
        }
    };
}

pub struct ObjectFormatHelper;

impl ObjectFormatHelper {
    pub fn encode_field<T: ?Sized>(obj: &mut Map<String, Value>, key: impl ToString, value: &T)
    where
        T: ObjectFormat,
    {
        obj.insert(key.to_string(), value.format_json());
    }

    pub fn encode_option_field<T>(
        obj: &mut Map<String, Value>,
        key: impl ToString,
        value: Option<&T>,
    ) where
        T: ObjectFormat,
    {
        if let Some(value) = value {
            obj.insert(key.to_string(), value.format_json());
        }
    }

    pub fn encode_array<T>(obj: &mut Map<String, Value>, key: impl ToString, list: &Vec<T>)
    where
        T: ObjectFormat,
    {
        obj.insert(key.to_string(), Self::encode_to_array(list));
    }

    pub fn encode_to_array<T>(list: &Vec<T>) -> Value
    where
        T: ObjectFormat,
    {
        let mut result = Vec::new();
        for item in list {
            let item = T::format_json(item);
            result.push(item);
        }

        Value::Array(result)
    }

    pub fn format_time(time: u64) -> String {
        use chrono::{DateTime, Utc};

        if time > 0 {
            let st = bucky_time_to_system_time(time);

            let now: DateTime<Utc> = st.into();
            let now = now.to_rfc3339();
            format!("{},{}", now, time)
        } else {
            "0".to_owned()
        }
    }
}

impl<T> ObjectFormat for NamedObjectDesc<T>
where
    T: DescContent + ObjectFormat,
    T::OwnerType: OwnerObj,
    T::AreaType: AreaObj,
    T::AuthorType: AuthorObj,
    T::PublicKeyType: PublicKeyObj,
    NamedObjectDesc<T>:
        ObjectDesc + OwnerObjectDesc + AreaObjectDesc + AuthorObjectDesc + PublicKeyObjectDesc,
{
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        let t = self.obj_type();
        JsonCodecHelper::encode_number_field(&mut map, "object_type", t);

        let object_id = self.calculate_id();
        JsonCodecHelper::encode_string_field(&mut map, "object_id", &object_id);

        JsonCodecHelper::encode_string_field(
            &mut map,
            "object_category",
            &object_id.object_category(),
        );

        JsonCodecHelper::encode_string_field_2(
            &mut map,
            "object_type_code",
            format!("{:?}", object_id.obj_type_code()),
        );

        if let Some(dec_id) = self.dec_id() {
            JsonCodecHelper::encode_string_field(&mut map, "dec_id", dec_id);
        }

        if let Some(ref_objs) = self.ref_objs() {
            ObjectFormatHelper::encode_array(&mut map, "ref_objects", ref_objs);
        }

        if let Some(prev) = self.prev() {
            JsonCodecHelper::encode_string_field(&mut map, "prev", prev);
        }

        if let Some(ts) = self.create_timestamp() {
            JsonCodecHelper::encode_string_field_2(
                &mut map,
                "create_timestamp",
                ts.to_hex_string(),
            );
        }

        JsonCodecHelper::encode_string_field_2(
            &mut map,
            "create_time",
            ObjectFormatHelper::format_time(self.create_time()),
        );

        if let Some(time) = self.expired_time() {
            JsonCodecHelper::encode_string_field_2(
                &mut map,
                "expired_time",
                ObjectFormatHelper::format_time(time),
            );
        }

        if let Some(owner) = self.owner() {
            JsonCodecHelper::encode_string_field(&mut map, "owner", owner);
        }

        if let Some(owner) = self.owner() {
            JsonCodecHelper::encode_string_field(&mut map, "owner", owner);
        }

        if let Some(area) = self.area() {
            ObjectFormatHelper::encode_field(&mut map, "area", area);
        }

        if let Some(pk) = self.public_key_ref() {
            ObjectFormatHelper::encode_field(&mut map, "public_key", &pk);
        }

        ObjectFormatHelper::encode_field(&mut map, "content", self.content());

        map.into()
    }
}

impl ObjectFormat for Area {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_number_field(&mut map, "country", self.country);
        JsonCodecHelper::encode_number_field(&mut map, "carrier", self.carrier);
        JsonCodecHelper::encode_number_field(&mut map, "city", self.city);
        JsonCodecHelper::encode_number_field(&mut map, "inner", self.inner);

        map.into()
    }
}

impl ObjectFormat for ObjectLink {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        JsonCodecHelper::encode_string_field(&mut map, "object_id", &self.obj_id);
        JsonCodecHelper::encode_option_string_field(&mut map, "owner", self.obj_owner.as_ref());

        map.into()
    }
}

impl<'a> ObjectFormat for PublicKeyRef<'a> {
    fn format_json(&self) -> serde_json::Value {
        match self {
            PublicKeyRef::Single(pk) => pk.format_json(),
            PublicKeyRef::MN(pk) => pk.format_json(),
        }
    }
}

impl ObjectFormat for PublicKeyValue {
    fn format_json(&self) -> serde_json::Value {
        self.as_ref().format_json()
    }
}

impl ObjectFormat for PublicKey {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "type", self.type_str());

        let raw = self.to_vec().unwrap();
        map.insert("raw_data".to_string(), Value::String(hex::encode(&raw)));

        map.into()
    }
}

impl ObjectFormat for MNPublicKey {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_number_field(&mut map, "index", self.0);
        ObjectFormatHelper::encode_array(&mut map, "value", &self.1);

        map.into()
    }
}

impl ObjectFormat for SignData {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "type", self.sign_type());

        map.insert(
            "raw_data".to_string(),
            Value::String(hex::encode(self.as_slice())),
        );

        map.into()
    }
}

impl ObjectFormat for SignatureSource {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match self {
            Self::RefIndex(index) => {
                JsonCodecHelper::encode_string_field(&mut map, "type", "ref_index");
                JsonCodecHelper::encode_number_field(&mut map, "value", *index);
            }
            Self::Object(link) => {
                JsonCodecHelper::encode_string_field(&mut map, "type", "object");
                ObjectFormatHelper::encode_field(&mut map, "value", link);
            }
            Self::Key(pk) => {
                JsonCodecHelper::encode_string_field(&mut map, "type", "key");
                ObjectFormatHelper::encode_field(&mut map, "value", pk);
            }
        }

        map.into()
    }
}

impl ObjectFormat for Signature {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        ObjectFormatHelper::encode_field(&mut map, "sign_source", self.sign_source());
        JsonCodecHelper::encode_number_field(
            &mut map,
            "sign_key_index",
            self.sign_key_index() as i32,
        );
        JsonCodecHelper::encode_string_field(&mut map, "sign_time", &self.sign_time());
        ObjectFormatHelper::encode_field(&mut map, "sign", self.sign());

        map.into()
    }
}

impl ObjectFormat for ObjectSigns {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        if let Some(signs) = self.desc_signs() {
            ObjectFormatHelper::encode_array(&mut map, "desc_signs", signs);
        }

        if let Some(signs) = self.body_signs() {
            ObjectFormatHelper::encode_array(&mut map, "body_signs", signs);
        }

        map.into()
    }
}

impl<B, O> ObjectFormat for ObjectMutBody<B, O>
where
    O: ObjectType,
    B: BodyContent + ObjectFormat,
{
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        if let Some(prev) = self.prev_version() {
            JsonCodecHelper::encode_string_field_2(&mut map, "prev_version", prev.to_hex_string());
        }

        JsonCodecHelper::encode_string_field_2(
            &mut map,
            "update_time",
            ObjectFormatHelper::format_time(self.update_time()),
        );

        ObjectFormatHelper::encode_field(&mut map, "content", self.content());

        if let Some(data) = self.user_data() {
            JsonCodecHelper::encode_string_field_2(&mut map, "user_data", hex::encode(data));
        }

        map.into()
    }
}

impl<O> ObjectFormat for NamedObjectBase<O>
where
    O: ObjectType,
    O::DescType: ObjectFormat,
    O::ContentType: BodyContent + ObjectFormat,
    NamedObjectBase<O>: NamedObject<O>,
{
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        ObjectFormatHelper::encode_field(&mut map, "desc", self.desc());

        if let Some(body) = self.body() {
            ObjectFormatHelper::encode_field(&mut map, "body", body);
        }

        let signs = self.signs();
        if !signs.is_empty() {
            ObjectFormatHelper::encode_field(&mut map, "signs", signs);
        }

        if let Some(nonce) = self.nonce() {
            JsonCodecHelper::encode_string_field(&mut map, "nonce", &nonce);
        }

        map.into()
    }
}

fn encode_content_codec(version: u8, format: u8) -> serde_json::Value {
    let mut map = serde_json::Map::new();

    JsonCodecHelper::encode_number_field(&mut map, "version", version);
    JsonCodecHelper::encode_number_field(&mut map, "format", format);

    map.into()
}

impl ObjectFormat for TypelessObjectDesc {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        let t = self.obj_type();
        JsonCodecHelper::encode_number_field(&mut map, "object_type", t);

        let object_id = self.calculate_id();
        JsonCodecHelper::encode_string_field(&mut map, "object_id", &object_id);

        JsonCodecHelper::encode_string_field(
            &mut map,
            "object_category",
            &object_id.object_category(),
        );

        JsonCodecHelper::encode_string_field_2(
            &mut map,
            "object_type_code",
            format!("{:?}", object_id.obj_type_code()),
        );

        if let Some(dec_id) = self.dec_id() {
            JsonCodecHelper::encode_string_field(&mut map, "dec_id", dec_id);
        }

        if let Some(ref_objs) = self.ref_objs() {
            ObjectFormatHelper::encode_array(&mut map, "ref_objects", ref_objs);
        }

        if let Some(prev) = self.prev() {
            JsonCodecHelper::encode_string_field(&mut map, "prev", prev);
        }

        if let Some(ts) = self.create_timestamp() {
            JsonCodecHelper::encode_string_field_2(
                &mut map,
                "create_timestamp",
                ts.to_hex_string(),
            );
        }

        JsonCodecHelper::encode_string_field(&mut map, "create_time", &self.create_time());

        if let Some(time) = self.expired_time() {
            JsonCodecHelper::encode_string_field(&mut map, "expired_time", &time);
        }

        if let Some(owner) = self.owner() {
            JsonCodecHelper::encode_string_field(&mut map, "owner", owner);
        }

        if let Some(owner) = self.owner() {
            JsonCodecHelper::encode_string_field(&mut map, "owner", owner);
        }

        if let Some(area) = self.area() {
            ObjectFormatHelper::encode_field(&mut map, "area", area);
        }

        if let Some(pk) = self.public_key() {
            ObjectFormatHelper::encode_field(&mut map, "public_key", &pk);
        }

        map.insert(
            "content_codec".to_string(),
            encode_content_codec(self.version(), self.format()),
        );
        info!("encode typeless core object to json");
        JsonCodecHelper::encode_string_field_2(&mut map, "content", hex::encode(self.content()));

        map.into()
    }
}

impl ObjectFormat for TypelessObjectBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        map.insert(
            "content_codec".to_string(),
            encode_content_codec(self.version(), self.format()),
        );
        JsonCodecHelper::encode_string_field_2(&mut map, "content", hex::encode(self.data()));

        map.into()
    }
}

impl ObjectFormat for StandardObject {
    fn format_json(&self) -> serde_json::Value {
        match self {
            StandardObject::Device(o) => o.format_json(),
            StandardObject::People(o) => o.format_json(),
            StandardObject::SimpleGroup(o) => o.format_json(),
            StandardObject::Org(o) => o.format_json(),
            StandardObject::AppGroup(o) => o.format_json(),
            StandardObject::UnionAccount(o) => o.format_json(),
            StandardObject::ChunkId(chunk_id) => chunk_id.format_json(),
            StandardObject::File(o) => o.format_json(),
            StandardObject::Dir(o) => o.format_json(),
            StandardObject::Diff(o) => o.format_json(),
            StandardObject::ProofOfService(o) => o.format_json(),
            StandardObject::Tx(o) => o.format_json(),
            StandardObject::Action(o) => o.format_json(),
            StandardObject::ObjectMap(o) => o.format_json(),
            StandardObject::Contract(o) => o.format_json(),
        }
    }
}

impl ObjectFormat for AnyNamedObject {
    fn format_json(&self) -> serde_json::Value {
        match self {
            AnyNamedObject::Standard(o) => o.format_json(),
            AnyNamedObject::Core(o) => o.format_json(),
            AnyNamedObject::DECApp(o) => o.format_json(),
        }
    }
}

// device
impl ObjectFormat for DeviceDescContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "unique_id", &self.unique_id());

        map.into()
    }
}

impl ObjectFormat for DeviceBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_str_array_field(&mut map, "endpoints", &self.endpoints());
        JsonCodecHelper::encode_str_array_field(&mut map, "sn_list", &self.sn_list());
        JsonCodecHelper::encode_str_array_field(
            &mut map,
            "passive_pn_list",
            &self.passive_pn_list(),
        );

        JsonCodecHelper::encode_option_string_field(&mut map, "name", self.name());

        map.into()
    }
}

// people
impl ObjectFormat for PeopleDescContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}

impl ObjectFormat for PeopleBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "ood_work_mode", &self.ood_work_mode());
        JsonCodecHelper::encode_str_array_field(&mut map, "ood_list", &self.ood_list());
        JsonCodecHelper::encode_option_string_field(&mut map, "name", self.name());
        JsonCodecHelper::encode_option_string_field(&mut map, "icon", self.icon());

        map.into()
    }
}

// simple group
impl ObjectFormat for SimpleGroupDescContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}

impl ObjectFormat for SimpleGroupBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_str_array_field(&mut map, "members", self.members());
        JsonCodecHelper::encode_str_array_field(&mut map, "ood_list", &self.ood_list());
        JsonCodecHelper::encode_string_field(&mut map, "ood_work_mode", &self.ood_work_mode());

        map.into()
    }
}

// org
impl ObjectFormat for OrgDescContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}

impl ObjectFormatAutoWithSerde for OrgBodyContent {}

// appgroup
impl ObjectFormat for AppGroupDescContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}
impl ObjectFormat for AppGroupBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}

// union account
impl ObjectFormatAutoWithSerde for UnionAccountDescContent {}

impl ObjectFormat for UnionAccountBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let map = serde_json::Map::new();

        map.into()
    }
}

// chunk_id
impl ObjectFormat for ChunkId {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "chunk_id", self);
        JsonCodecHelper::encode_string_field(&mut map, "len", &self.len());
        JsonCodecHelper::encode_string_field_2(&mut map, "hash_value", hex::encode(self.hash()));

        map.into()
    }
}

// file
impl ObjectFormat for FileDescContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "len", &self.len());
        JsonCodecHelper::encode_string_field_2(&mut map, "hash_value", self.hash().to_hex_string());

        map.into()
    }
}

impl ObjectFormat for ChunkBundle {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_str_array_field(&mut map, "list", self.chunk_list());
        JsonCodecHelper::encode_string_field(&mut map, "hash_method", self.hash_method().as_str());

        map.into()
    }
}

impl ObjectFormat for ChunkList {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match self {
            Self::ChunkInList(list) => {
                JsonCodecHelper::encode_str_array_field(&mut map, "chunk_in_list", list);
            }
            Self::ChunkInFile(file_id) => {
                JsonCodecHelper::encode_string_field(&mut map, "chunk_in_file", &file_id);
            }
            Self::ChunkInBundle(bundle) => {
                ObjectFormatHelper::encode_field(&mut map, "chunk_in_bundle", bundle);
            }
        }

        map.into()
    }
}

impl ObjectFormat for FileBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        ObjectFormatHelper::encode_field(&mut map, "chunk_list", self.chunk_list());

        map.into()
    }
}

// dir
impl ObjectFormat for Attributes {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_number_field(&mut map, "flags", self.flags());

        map.into()
    }
}

impl ObjectFormat for InnerNode {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match self {
            Self::ObjId(id) => {
                JsonCodecHelper::encode_string_field(&mut map, "object_id", id);
            }
            Self::Chunk(id) => {
                JsonCodecHelper::encode_string_field(&mut map, "chunk", id);
            }
            Self::IndexInParentChunk(begin, end) => {
                let range = format!("[{},{})", begin, end);
                JsonCodecHelper::encode_string_field_2(&mut map, "index_in_parent_chunk", range);
            }
        }

        map.into()
    }
}

impl ObjectFormat for DirBodyDescObjectMap {
    fn format_json(&self) -> serde_json::Value {
        let mut list = Vec::with_capacity(self.len());

        for (key, value) in self {
            let mut map = serde_json::Map::new();
            JsonCodecHelper::encode_string_field(&mut map, "name", key);
            ObjectFormatHelper::encode_field(&mut map, "attributes", value.attributes());
            ObjectFormatHelper::encode_field(&mut map, "node", value.node());

            list.push(serde_json::Value::Object(map));
        }

        serde_json::Value::Array(list)
    }
}

impl ObjectFormat for NDNObjectList {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        if let Some(parent_chunk) = self.parent_chunk() {
            ObjectFormatHelper::encode_field(&mut map, "parent_chunk", parent_chunk);
        }

        ObjectFormatHelper::encode_field(&mut map, "object_map", self.object_map());
        map.into()
    }
}

impl ObjectFormat for NDNObjectInfo {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match self {
            Self::Chunk(chunk_id) => {
                ObjectFormatHelper::encode_field(&mut map, "chunk", chunk_id);
            }
            Self::ObjList(list) => {
                ObjectFormatHelper::encode_field(&mut map, "object_list", list);
            }
        }
        map.into()
    }
}

impl ObjectFormat for DirDescContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        ObjectFormatHelper::encode_field(&mut map, "attributes", self.attributes());
        ObjectFormatHelper::encode_field(&mut map, "object_list", self.obj_list());

        map.into()
    }
}

impl ObjectFormat for DirBodyContentObjectList {
    fn format_json(&self) -> serde_json::Value {
        let list: Vec<ObjectId> = self.iter().map(|(key, _)| key.to_owned()).collect();

        serde_json::to_value(list).unwrap()
    }
}

impl ObjectFormat for DirBodyContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        match self {
            Self::Chunk(chunk_id) => ObjectFormatHelper::encode_field(&mut map, "chunk", chunk_id),
            Self::ObjList(list) => {
                ObjectFormatHelper::encode_field(&mut map, "object_list", list);
            }
        }

        map.into()
    }
}

// diff
impl ObjectFormat for DiffDescContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "file_id", self.file_id());
        JsonCodecHelper::encode_str_array_field(&mut map, "diff_list", self.diff_list());

        map.into()
    }
}
object_format_empty_impl!(DiffBodyContent);

// ProofOfService
object_format_not_impl!(ProofOfServiceDescContent<ProofData>);
object_format_not_impl!(ProofOfServiceBodyContent<ProofData>);

// Tx
object_format_not_impl!(TxDescContent<TxBody>);
object_format_not_impl!(TxBodyContent);

// action
object_format_empty_impl!(ActionDescContent);
object_format_empty_impl!(ActionBodyContent);

// Contract
object_format_not_impl!(ContractDescContent<ContractData>);
object_format_not_impl!(ContractBodyContent<ContractData>);

// ObjectMap
impl ObjectFormat for SimpleContent {
    fn format_json(&self) -> serde_json::Value {
        match self {
            Self::Map(content) => serde_json::to_value(content.values()).unwrap(),
            Self::DiffMap(content) => serde_json::to_value(content.values()).unwrap(),
            Self::Set(content) => serde_json::to_value(content.values()).unwrap(),
            Self::DiffSet(content) => serde_json::to_value(content.values()).unwrap(),
        }
    }
}

impl ObjectFormat for ObjectMapSimpleContent {
    fn format_json(&self) -> serde_json::Value {
        self.content().format_json()
    }
}

impl ObjectFormat for ObjectMapHubContent {
    fn format_json(&self) -> serde_json::Value {
        serde_json::to_value(self.subs()).unwrap()
    }
}

impl ObjectFormat for ObjectMapContent {
    fn format_json(&self) -> serde_json::Value {
        match self {
            Self::Simple(content) => content.format_json(),
            Self::Hub(content) => content.format_json(),
        }
    }
}

impl ObjectFormat for ObjectMapDescContent {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();

        JsonCodecHelper::encode_string_field(&mut map, "class", self.class().as_str());

        JsonCodecHelper::encode_string_field(&mut map, "count", &self.count());
        JsonCodecHelper::encode_string_field(&mut map, "size", &self.size());

        JsonCodecHelper::encode_number_field(&mut map, "depth", self.depth());

        JsonCodecHelper::encode_string_field_2(
            &mut map,
            "content_type",
            self.content_type().to_string(),
        );
        JsonCodecHelper::encode_string_field_2(&mut map, "content_mode", self.mode().to_string());

        ObjectFormatHelper::encode_field(&mut map, "content", self.content());

        map.into()
    }
}

object_format_empty_impl!(ObjectMapBodyContent);

impl ObjectFormatAutoWithSerde for EmptyProtobufBodyContent {}
impl ObjectFormatAutoWithSerde for EmptyBodyContent {}

#[test]
fn test() {
    let owner = ObjectId::default();
    let hash = HashValue::default();

    let chunk_list = vec![ChunkId::default(), ChunkId::default()];

    let chunk_list = ChunkList::ChunkInList(chunk_list);

    let file = File::new(owner, 100, hash, chunk_list)
        .no_create_time()
        .build();

    let value = file.desc().format_json();
    let s = value.to_string();
    println!("{}", s);
}


use std::sync::{Arc, Mutex};
use std::collections::{hash_map::Entry, HashMap};

pub fn format_json<T: for<'de> RawDecode<'de> + ObjectFormat>(buf: &[u8]) -> BuckyResult<serde_json::Value> {
    let (obj, _) = T::raw_decode(buf)?;

    Ok(obj.format_json())
}


pub struct FormatFactory {
    ext_types: Arc<
        Mutex<
            HashMap<u16, Arc<Box<dyn Fn(&[u8]) -> BuckyResult<serde_json::Value> + Send + Sync>>>,
        >,
    >,
}

impl FormatFactory {
    pub fn new() -> Self {
        Self {
            ext_types: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register<F: 'static + Fn(&[u8]) -> BuckyResult<serde_json::Value> + Send + Sync>(
        &self,
        obj_type: impl Into<u16>,
        formater: F,
    ) {
        let f = Arc::new(Box::new(formater)
            as Box<dyn Fn(&[u8]) -> BuckyResult<serde_json::Value> + Send + Sync>);

        let mut all = self.ext_types.lock().unwrap();
        match all.entry(obj_type.into()) {
            Entry::Vacant(v) => {
                v.insert(f);
            }
            Entry::Occupied(_o) => {
                unreachable!();
            }
        }
    }

    pub fn format(&self, obj_type: u16, obj_raw: &[u8]) -> Option<serde_json::Value> {
        let f = self
            .ext_types
            .lock()
            .unwrap()
            .get(&obj_type)
            .map(|f| f.clone());
        match f {
            Some(f) => match f(obj_raw) {
                Ok(r) => Some(r),
                Err(_e) => None,
            },
            None => None,
        }
    }
}


lazy_static::lazy_static! {
    pub static ref FORMAT_FACTORY: FormatFactory = FormatFactory::new();
}