use crate::*;

#[derive(Debug, Clone)]
pub enum AnyNamedObject {
    Standard(StandardObject),
    Core(TypelessCoreObject),
    DECApp(TypelessDECAppObject),
}

#[macro_export]
macro_rules! match_any_obj {
    ($on:ident, $o:ident, $body:tt, $chunk_id:ident, $chunk_body:tt) => {
        match $on {
            AnyNamedObject::Standard(o) => match o {
                StandardObject::Device($o) => $body,
                StandardObject::People($o) => $body,
                StandardObject::SimpleGroup($o) => $body,
                StandardObject::Org($o) => $body,
                StandardObject::AppGroup($o) => $body,
                StandardObject::UnionAccount($o) => $body,
                StandardObject::ChunkId($chunk_id) => $chunk_body,
                StandardObject::File($o) => $body,
                StandardObject::Dir($o) => $body,
                StandardObject::Diff($o) => $body,
                StandardObject::ProofOfService($o) => $body,
                StandardObject::Tx($o) => $body,
                StandardObject::Action($o) => $body,
                StandardObject::ObjectMap($o) => $body,
                StandardObject::Contract($o) => $body,
            },
            AnyNamedObject::Core($o) => $body,
            AnyNamedObject::DECApp($o) => $body,
        }
    };
}

impl AnyNamedObject {
    pub fn object_id(&self) -> ObjectId {
        self.calculate_id()
    }
    pub fn calculate_id(&self) -> ObjectId {
        match_any_obj!(self, o, { o.desc().calculate_id() }, chunk_id, {
            chunk_id.object_id()
        })
    }

    pub fn try_clone(&self) -> BuckyResult<Self> {
        let len = self.raw_measure(&None).map_err(|e| {
            log::error!("AnyNamedObject::try_clone/raw_measure error:{}", e);
            e
        })?;

        let mut buf = Vec::with_capacity(len);
        unsafe {
            buf.set_len(len);
        }

        self.raw_encode(&mut buf[..], &None).map_err(|e| {
            log::error!("AnyNamedObject::try_clone/raw_encode error:{}", e);
            e
        })?;

        let (ret, _) = Self::raw_decode(&buf[..]).map_err(|e| {
            log::error!("AnyNamedObject::try_clone/raw_decode error:{}", e);
            e
        })?;

        Ok(ret)
    }

    pub fn obj_type(&self) -> u16 {
        match_any_obj!(self, o, { o.desc().obj_type() }, _chunk_id, {
            ObjectTypeCode::Chunk.to_u16()
        })
    }

    pub fn obj_type_code(&self) -> ObjectTypeCode {
        match_any_obj!(self, o, { o.desc().obj_type_code() }, _chunk_id, {
            ObjectTypeCode::Chunk
        })
    }

    pub fn dec_id(&self) -> &Option<ObjectId> {
        match_any_obj!(self, o, { o.desc().dec_id() }, _chunk_id, { &None })
    }

    pub fn owner(&self) -> &Option<ObjectId> {
        match self {
            AnyNamedObject::Standard(s) => s.owner(),
            AnyNamedObject::Core(c) => c.desc().owner(),
            AnyNamedObject::DECApp(d) => d.desc().owner(),
        }
    }

    pub fn public_key(&self) -> Option<PublicKeyRef> {
        match self {
            AnyNamedObject::Standard(s) => s.public_key(),
            AnyNamedObject::Core(o) => o.desc().public_key(),
            AnyNamedObject::DECApp(o) => o.desc().public_key(),
        }
    }

    pub fn author(&self) -> &Option<ObjectId> {
        match self {
            AnyNamedObject::Standard(s) => s.author(),
            AnyNamedObject::Core(c) => c.desc().author(),
            AnyNamedObject::DECApp(d) => d.desc().author(),
        }
    }

    pub fn prev(&self) -> &Option<ObjectId> {
        match self {
            AnyNamedObject::Standard(s) => s.prev(),
            AnyNamedObject::Core(c) => c.desc().prev(),
            AnyNamedObject::DECApp(d) => d.desc().prev(),
        }
    }

    pub fn ood_list(&self) -> BuckyResult<&Vec<DeviceId>> {
        match self {
            AnyNamedObject::Standard(s) => s.ood_list(),
            AnyNamedObject::Core(_c) => Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                "ood_list not support in typeless Core object",
            )),
            AnyNamedObject::DECApp(_d) => Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                "ood_list not support in typeless DECApp object",
            )),
        }
    }

    pub fn ood_work_mode(&self) -> BuckyResult<OODWorkMode> {
        match self {
            AnyNamedObject::Standard(s) => s.ood_work_mode(),
            AnyNamedObject::Core(_c) => Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                "ood_work_mode not support in typeless Core object",
            )),
            AnyNamedObject::DECApp(_d) => Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                "ood_work_mode not support in typeless DECApp object",
            )),
        }
    }

    pub fn signs(&self) -> Option<&ObjectSigns> {
        match_any_obj!(self, o, { Some(o.signs()) }, chunk_id, {
            error!("chunk has no signs: {}", chunk_id);

            None
        })
    }

    pub fn signs_mut(&mut self) -> Option<&mut ObjectSigns> {
        match_any_obj!(self, o, { Some(o.signs_mut()) }, chunk_id, {
            error!("chunk has no signs: {}", chunk_id);

            None
        })
    }

    pub fn desc_hash(&self) -> BuckyResult<HashValue> {
        match_any_obj!(self, o, { o.desc().raw_hash_value() }, chunk_id, {
            let msg = format!("chunk has no desc: {}", chunk_id);
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
        })
    }

    pub fn has_body(&self) -> BuckyResult<bool> {
        match_any_obj!(self, o, { Ok(o.body().is_some()) }, _chunk_id, {
            let msg = format!("chunk has no body: {}", self.calculate_id());
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
        })
    }

    pub fn body_hash(&self) -> BuckyResult<Option<HashValue>> {
        match_any_obj!(
            self,
            o,
            {
                if o.body().is_some() {
                    let hash_value = o.body().as_ref().unwrap().raw_hash_value()?;
                    Ok(Some(hash_value))
                } else {
                    Ok(None)
                }
            },
            _chunk_id,
            {
                let msg = format!("chunk has no body: {}", self.calculate_id());
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        )
    }

    pub fn ref_objs(&self) -> Option<&Vec<ObjectLink>> {
        match_any_obj!(self, o, { o.desc().ref_objs().as_ref() }, chunk_id, {
            error!("chunk has no ref_objs: {}", chunk_id);

            None
        })
    }

    pub fn is_standard(&self) -> bool {
        match self {
            AnyNamedObject::Standard(_) => true,
            _ => false,
        }
    }

    pub fn is_core(&self) -> bool {
        match self {
            AnyNamedObject::Core(_) => true,
            _ => false,
        }
    }

    pub fn is_dec(&self) -> bool {
        match self {
            AnyNamedObject::DECApp(_) => true,
            _ => false,
        }
    }

    // reset the object's body with the same obj_type object
    pub fn set_body_expect(&mut self, other: &Self) {
        assert_eq!(self.obj_type(), other.obj_type());
        
        match self {
            Self::Standard(o) => {
                match other {
                    Self::Standard(other) => {
                        o.set_body_expect(other);
                    }
                    _ => unreachable!(),
                }
            }
            Self::Core(o) => {
                match other {
                    Self::Core(other) => {
                        *o.body_mut() = other.body().to_owned();
                    }
                    _ => unreachable!(),
                }
            }
            Self::DECApp(o) => {
                match other {
                    Self::DECApp(other) => {
                        *o.body_mut() = other.body().to_owned();
                    }
                    _ => unreachable!(),
                }
            }
        }
    }

    // 设置对象body的修改时间
    pub fn set_body_update_time(&mut self, time: u64) {
        match_any_obj!(
            self,
            o,
            {
                match o.body_mut().as_mut() {
                    Some(body) => body.set_update_time(time),
                    None => {}
                }
            },
            _chunk_id,
            {}
        )
    }

    pub fn create_time(&self) -> u64 {
        match_any_obj!(self, o, { o.desc().create_time() }, _chunk_id, { 0 })
    }

    pub fn expired_time(&self) -> Option<u64> {
        match_any_obj!(self, o, { o.desc().expired_time() }, _chunk_id, { None })
    }

    pub fn update_time(&self) -> Option<u64> {
        match_any_obj!(
            self,
            o,
            {
                match o.body().as_ref() {
                    Some(body) => Some(body.update_time()),
                    None => None,
                }
            },
            _chunk_id,
            { None }
        )
    }

    // 获取对象body的修改时间(不包括签名)
    pub fn get_update_time(&self) -> u64 {
        match_any_obj!(
            self,
            o,
            {
                match o.body().as_ref() {
                    Some(body) => body.update_time(),
                    None => 0_u64,
                }
            },
            _chunk_id,
            { 0 }
        )
    }

    // 获取body+signs的最新修改时间
    pub fn get_full_update_time(&self) -> u64 {
        let update_time = self.get_update_time();

        // 如果签名时间比较新，那么取签名时间
        let latest_sign_time = match self.signs() {
            Some(v) => v.latest_sign_time(),
            None => 0,
        };

        std::cmp::max(update_time, latest_sign_time)
    }
}


impl RawEncode for AnyNamedObject {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            AnyNamedObject::Standard(obj) => obj.raw_measure(purpose),
            AnyNamedObject::Core(obj) => obj.raw_measure(purpose),
            AnyNamedObject::DECApp(obj) => obj.raw_measure(purpose),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            AnyNamedObject::Standard(obj) => obj.raw_encode(buf, purpose),
            AnyNamedObject::Core(obj) => obj.raw_encode(buf, purpose),
            AnyNamedObject::DECApp(obj) => obj.raw_encode(buf, purpose),
        }
    }
}

impl<'de> RawDecode<'de> for AnyNamedObject {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (obj_type_info, _new_buffer) = NamedObjectContext::raw_decode(buf).map_err(|e| {
            log::error!("AnyNamedObject::raw_decode/obj_type_info error:{}", e);
            e
        })?;

        match obj_type_info.obj_type_code() {
            ObjectTypeCode::Device => {
                let (device, buf) = Device::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/device error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::Device(device)),
                    buf,
                ));
            }
            ObjectTypeCode::People => {
                let (people, buf) = People::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/people error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::People(people)),
                    buf,
                ));
            }
            ObjectTypeCode::Org => {
                let (org, buf) = Org::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/org error:{}", e);
                    e
                })?;
                return Ok((AnyNamedObject::Standard(StandardObject::Org(org)), buf));
            }
            ObjectTypeCode::AppGroup => {
                let (app_group, buf) = AppGroup::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/app_group error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::AppGroup(app_group)),
                    buf,
                ));
            }
            ObjectTypeCode::SimpleGroup => {
                let (simple_group, buf) = SimpleGroup::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/simple_group error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::SimpleGroup(simple_group)),
                    buf,
                ));
            }
            ObjectTypeCode::UnionAccount => {
                let (ua, buf) = UnionAccount::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/ua error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::UnionAccount(ua)),
                    buf,
                ));
            }
            ObjectTypeCode::Chunk => {
                unreachable!();
            }
            ObjectTypeCode::File => {
                let (file, buf) = File::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/file error:{}", e);
                    e
                })?;
                return Ok((AnyNamedObject::Standard(StandardObject::File(file)), buf));
            }
            ObjectTypeCode::Dir => {
                let (dir, buf) = Dir::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/dir error:{}", e);
                    e
                })?;
                return Ok((AnyNamedObject::Standard(StandardObject::Dir(dir)), buf));
            }
            ObjectTypeCode::Diff => {
                let (diff, buf) = Diff::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/diff error:{}", e);
                    e
                })?;
                return Ok((AnyNamedObject::Standard(StandardObject::Diff(diff)), buf));
            }
            ObjectTypeCode::ProofOfService => {
                let (prof, buf) = ProofOfService::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/prof error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::ProofOfService(prof)),
                    buf,
                ));
            }
            ObjectTypeCode::Tx => {
                let (tx, buf) = Tx::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/tx error:{}", e);
                    e
                })?;
                return Ok((AnyNamedObject::Standard(StandardObject::Tx(tx)), buf));
            }
            ObjectTypeCode::Action => {
                let (action, buf) = Action::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/action error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::Action(action)),
                    buf,
                ));
            }
            ObjectTypeCode::ObjectMap => {
                let (relation, buf) = ObjectMap::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/relation error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::ObjectMap(relation)),
                    buf,
                ));
            }
            ObjectTypeCode::Contract => {
                let (contract, buf) = Contract::raw_decode(buf).map_err(|e| {
                    log::error!("AnyNamedObject::raw_decode/contract error:{}", e);
                    e
                })?;
                return Ok((
                    AnyNamedObject::Standard(StandardObject::Contract(contract)),
                    buf,
                ));
            }
            ObjectTypeCode::Custom => {
                return if obj_type_info.is_decapp_object() {
                    // println!("is dec app object");

                    let (dec_obj, buf) = TypelessDECAppObject::raw_decode(buf)?;
                    Ok((AnyNamedObject::DECApp(dec_obj), buf))
                } else {
                    // println!("is core object");

                    let (core_obj, buf) = TypelessCoreObject::raw_decode(buf)?;
                    Ok((AnyNamedObject::Core(core_obj), buf))
                };
            }
        }
    }
}

// 用 base16 hex实现serde
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

impl Serialize for AnyNamedObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let len = self.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; len];
        self.raw_encode(buf.as_mut_slice(), &None).unwrap();
        serializer.serialize_str(&hex::encode(buf))
    }
}

impl<'de> Deserialize<'de> for AnyNamedObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawObjectIdVisitor;
        impl<'de> Visitor<'de> for RawObjectIdVisitor {
            type Value = AnyNamedObject;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "{}", "an ObjectId")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let raw = hex::decode(v)
                    .map_err(|err| E::custom(err.to_string()))
                    .map_err(|e| {
                        log::error!("AnyNamedObject::Deserialize error:{}", e);
                        e
                    })?;
                AnyNamedObject::raw_decode(raw.as_slice())
                    .map_err(|err| E::custom(err.to_string()))
                    .map(|(obj, _)| obj)
            }
        }
        deserializer.deserialize_str(RawObjectIdVisitor)
    }
}

use std::sync::Arc;
impl Into<AnyNamedObject> for Arc<AnyNamedObject> {
    fn into(self) -> AnyNamedObject {
        match Arc::try_unwrap(self) {
            Ok(v) => v,
            Err(v) => v.as_ref().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use std::str::FromStr;

    #[test]
    fn test_any() {
        let mut sn_list = Vec::new();
        let mut endpoints = Vec::new();
        let unique_id = UniqueId::default();
        let name = "test_device";
        let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();

        let ep = Endpoint::from_str("W4udp120.24.6.201:8060").unwrap();
        for _ in 0..10 {
            endpoints.push(ep.clone());
        }
        let device_id2 =
            DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();
        for _ in 0..10 {
            sn_list.push(device_id2.clone());
        }
        let desc_content = DeviceDescContent::new(unique_id.clone());

        let body_content =
            DeviceBodyContent::new(endpoints, sn_list, Vec::new(), Some(name.to_owned()));
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = secret1.public();

        let device = DeviceBuilder::new(desc_content, body_content)
            .public_key(public_key.clone())
            //.area(area.clone().unwrap())
            .owner(owner.clone())
            .build();

        let device_id = device.desc().device_id().object_id().to_owned();

        let buf = device.to_vec().unwrap();
        let (obj, _buf) = AnyNamedObject::raw_decode(&buf).unwrap();
        println!("{:?}", obj.owner().unwrap());
        assert_eq!(obj.owner().to_owned().unwrap(), owner);
        assert_eq!(obj.calculate_id(), device_id);
        let pk = obj.public_key().unwrap();
        if let PublicKeyRef::Single(key) = pk {
            assert_eq!(*key, public_key);
        } else {
            unreachable!();
        }

        let buf2 = obj.to_vec().unwrap();
        assert_eq!(buf.len(), buf2.len());
        assert_eq!(buf, buf2);
    }
}



macro_rules! any_for_standard_target {
    ($as_name:ident, $into_name:ident, $target:ident) => {
        impl AnyNamedObject {
            pub fn $as_name(&self) -> &$target {
                match self {
                    AnyNamedObject::Standard(s) => {
                        match s {
                            StandardObject::$target(f) => f,
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            pub fn $into_name(self) -> $target {
                match self {
                    AnyNamedObject::Standard(s) => {
                        match s {
                            StandardObject::$target(f) => f,
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

any_for_standard_target!(as_file, into_file, File);
any_for_standard_target!(as_dir, into_dir, Dir);
any_for_standard_target!(as_people, into_people, People);
any_for_standard_target!(as_device, into_device, Device);
any_for_standard_target!(as_simple_group, into_simple_group, SimpleGroup);
any_for_standard_target!(as_object_map, into_object_map, ObjectMap);