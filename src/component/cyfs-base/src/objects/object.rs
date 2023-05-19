use crate::*;

use std::fmt;
use std::marker::PhantomData;

pub trait ObjectDesc {
    fn obj_type(&self) -> u16;

    // Default implementation, from obj_type to obj_type_code
    fn obj_type_code(&self) -> ObjectTypeCode {
        let t = self.obj_type();
        t.into()
    }

    fn is_standard_object(&self) -> bool {
        let c = self.obj_type_code();
        c != ObjectTypeCode::Custom
    }

    fn is_core_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom && object_type_helper::is_core_object(t)
    }

    fn is_dec_app_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom && object_type_helper::is_dec_app_object(t)
    }

    fn object_id(&self) -> ObjectId {
        self.calculate_id()
    }
    // calculate object_id with desc
    fn calculate_id(&self) -> ObjectId;

    // Get the dec-id(object-id) of the DECApp to which it belongs
    fn dec_id(&self) -> &Option<ObjectId>;

    // List of linked objects
    fn ref_objs(&self) -> &Option<Vec<ObjectLink>>;

    // Previous version number
    fn prev(&self) -> &Option<ObjectId>;

    // The associated hash at the time of creation, e.g. BTC Transaction hash
    fn create_timestamp(&self) -> &Option<HashValue>;

    // Created timestamp, or return 0 if it does not exist
    fn create_time(&self) -> u64;
    fn option_create_time(&self) -> Option<u64>;

    // Expiration timestamp
    fn expired_time(&self) -> Option<u64>;
}

/// Authorized-Object, maybe oneof PublicKey::Single or PublicKey::MN
pub trait PublicKeyObjectDesc {
    fn public_key_ref(&self) -> Option<PublicKeyRef>;
}

/// Single public key Authorized object, explicitly using the PublicKey::Single type
/// The object that implements the Trait must also implement the PublicKeyObjectDesc
pub trait SingleKeyObjectDesc: PublicKeyObjectDesc {
    fn public_key(&self) -> &PublicKey;
}

/// Multi public key Authorized object, explicitly using the PublicKey::MN type
/// The object that implements the Trait must also implement the PublicKeyObjectDesc
pub trait MNKeyObjectDesc: PublicKeyObjectDesc {
    fn mn_public_key(&self) -> &MNPublicKey;
}

/// Owned-Object
pub trait OwnerObjectDesc {
    fn owner(&self) -> &Option<ObjectId>;
}

/// Object with author
pub trait AuthorObjectDesc {
    fn author(&self) -> &Option<ObjectId>;
}

/// Object with area
pub trait AreaObjectDesc {
    fn area(&self) -> &Option<Area>;
}

// obj_flags: u16
// ========
// * The first 5 bits are used to indicate the codec status and are not counted in the hash calculation. (Always fill in 0 when calculating)
// * The remaining 11bits are used to identify the desc header
//
// Object segment codec flags
// --------
// 0:  If encrypted crypto (now the encryption structure is not defined, must fill in 0)
// 1:  If contains mut_body
// 2:  If contains desc_signs
// 3:  If contains body_signs
// 4:  If contains nonce
//
// ObjectDesc codec flags
// --------
// 5:  If contains dec_id
// 6:  If contains ref_objecs
// 7:  If contains prev
// 8:  If contains create_timestamp
// 9:  If contains create_time
// 10: If contains expired_time
//
// OwnerObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc 标志位
// ---------
// 11: If contains owner
// 12: If contains area
// 13: If contains author
// 14: If contains public_key
// 15: If contains ext
pub const OBJECT_FLAG_CTYPTO: u16 = 0x01;
pub const OBJECT_FLAG_MUT_BODY: u16 = 0x01 << 1;
pub const OBJECT_FLAG_DESC_SIGNS: u16 = 0x01 << 2;
pub const OBJECT_FLAG_BODY_SIGNS: u16 = 0x01 << 3;
pub const OBJECT_FLAG_NONCE: u16 = 0x01 << 4;

pub const OBJECT_FLAG_DESC_ID: u16 = 0x01 << 5;
pub const OBJECT_FLAG_REF_OBJECTS: u16 = 0x01 << 6;
pub const OBJECT_FLAG_PREV: u16 = 0x01 << 7;
pub const OBJECT_FLAG_CREATE_TIMESTAMP: u16 = 0x01 << 8;
pub const OBJECT_FLAG_CREATE_TIME: u16 = 0x01 << 9;
pub const OBJECT_FLAG_EXPIRED_TIME: u16 = 0x01 << 10;

pub const OBJECT_FLAG_OWNER: u16 = 0x01 << 11;
pub const OBJECT_FLAG_AREA: u16 = 0x01 << 12;
pub const OBJECT_FLAG_AUTHOR: u16 = 0x01 << 13;
pub const OBJECT_FLAG_PUBLIC_KEY: u16 = 0x01 << 14;

// If contains the ext field, reserved for the non-DescContent part of the extension, including a u16 length + the corresponding content
pub const OBJECT_FLAG_EXT: u16 = 0x01 << 15;

// Object type interval definition, [start, end)
pub const OBJECT_TYPE_ANY: u16 = 0;
pub const OBJECT_TYPE_STANDARD_START: u16 = 1;
pub const OBJECT_TYPE_STANDARD_END: u16 = 16;
pub const OBJECT_TYPE_CORE_START: u16 = 17;
pub const OBJECT_TYPE_CORE_END: u16 = 32767;
pub const OBJECT_TYPE_DECAPP_START: u16 = 32768;
pub const OBJECT_TYPE_DECAPP_END: u16 = 65535;

pub const OBJECT_PUBLIC_KEY_NONE: u8 = 0x00;
pub const OBJECT_PUBLIC_KEY_SINGLE: u8 = 0x01;
pub const OBJECT_PUBLIC_KEY_MN: u8 = 0x02;

pub const OBJECT_BODY_FLAG_PREV: u8 = 0x01;
pub const OBJECT_BODY_FLAG_USER_DATA: u8 = 0x01 << 1;

// Whether include extended fields, in the same format as desc
pub const OBJECT_BODY_FLAG_EXT: u8 = 0x01 << 2;

#[derive(Clone, Debug)]
pub struct NamedObjectBodyContext {
    body_content_cached_size: Option<usize>,
}

impl NamedObjectBodyContext {
    pub fn new() -> Self {
        Self {
            body_content_cached_size: None,
        }
    }

    pub fn cache_body_content_size(&mut self, size: usize) -> &mut Self {
        assert!(self.body_content_cached_size.is_none());
        self.body_content_cached_size = Some(size);
        self
    }

    pub fn get_body_content_cached_size(&self) -> usize {
        self.body_content_cached_size.unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct NamedObjectContext {
    obj_type: u16,
    obj_flags: u16,
    obj_type_code: ObjectTypeCode,

    // DescContent缓存的大小
    desc_content_cached_size: Option<u16>,

    body_context: NamedObjectBodyContext,
}

impl NamedObjectContext {
    pub fn new(obj_type: u16, obj_flags: u16) -> Self {
        let obj_type_code = obj_type.into();
        Self {
            obj_type,
            obj_flags,
            obj_type_code,
            desc_content_cached_size: None,
            body_context: NamedObjectBodyContext::new(),
        }
    }

    pub fn obj_type_code(&self) -> ObjectTypeCode {
        self.obj_type_code.clone()
    }

    pub fn obj_type(&self) -> u16 {
        self.obj_type
    }

    pub fn obj_flags(&self) -> u16 {
        self.obj_flags
    }

    pub fn is_standard_object(&self) -> bool {
        object_type_helper::is_standard_object(self.obj_type)
    }

    pub fn is_core_object(&self) -> bool {
        object_type_helper::is_core_object(self.obj_type)
    }

    pub fn is_decapp_object(&self) -> bool {
        object_type_helper::is_dec_app_object(self.obj_type)
    }

    //
    // common
    //

    pub fn with_crypto(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CTYPTO;
        self
    }

    pub fn has_crypto(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CTYPTO)
    }

    pub fn with_mut_body(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_MUT_BODY;
        self
    }

    pub fn has_mut_body(&self) -> bool {
        self.has_flag(OBJECT_FLAG_MUT_BODY)
    }

    pub fn with_desc_signs(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_DESC_SIGNS;
        self
    }

    pub fn has_desc_signs(&self) -> bool {
        self.has_flag(OBJECT_FLAG_DESC_SIGNS)
    }

    pub fn with_body_signs(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_BODY_SIGNS;
        self
    }

    pub fn has_body_signs(&self) -> bool {
        self.has_flag(OBJECT_FLAG_BODY_SIGNS)
    }

    pub fn with_nonce(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_NONCE;
        self
    }

    pub fn has_nonce(&self) -> bool {
        self.has_flag(OBJECT_FLAG_NONCE)
    }

    //
    // ObjectDesc
    //

    pub fn with_dec_id(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_DESC_ID;
        self
    }

    pub fn has_dec_id(&self) -> bool {
        self.has_flag(OBJECT_FLAG_DESC_ID)
    }

    pub fn with_ref_objects(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_REF_OBJECTS;
        self
    }

    pub fn has_ref_objects(&self) -> bool {
        self.has_flag(OBJECT_FLAG_REF_OBJECTS)
    }

    pub fn with_prev(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_PREV;
        self
    }

    pub fn has_prev(&self) -> bool {
        self.has_flag(OBJECT_FLAG_PREV)
    }

    pub fn with_create_timestamp(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CREATE_TIMESTAMP;
        self
    }

    pub fn has_create_time_stamp(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CREATE_TIMESTAMP)
    }

    pub fn with_create_time(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CREATE_TIME;
        self
    }

    pub fn has_create_time(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CREATE_TIME)
    }

    pub fn with_expired_time(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_EXPIRED_TIME;
        self
    }

    pub fn has_expired_time(&self) -> bool {
        self.has_flag(OBJECT_FLAG_EXPIRED_TIME)
    }

    //
    // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
    //

    pub fn with_owner(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_OWNER;
        self
    }

    pub fn has_owner(&self) -> bool {
        self.has_flag(OBJECT_FLAG_OWNER)
    }

    pub fn with_area(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_AREA;
        self
    }

    pub fn has_area(&self) -> bool {
        self.has_flag(OBJECT_FLAG_AREA)
    }

    pub fn with_public_key(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_PUBLIC_KEY;
        self
    }

    pub fn has_public_key(&self) -> bool {
        self.has_flag(OBJECT_FLAG_PUBLIC_KEY)
    }

    pub fn with_author(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_AUTHOR;
        self
    }

    pub fn has_author(&self) -> bool {
        self.has_flag(OBJECT_FLAG_AUTHOR)
    }

    pub fn has_ext(&self) -> bool {
        self.has_flag(OBJECT_FLAG_EXT)
    }

    // desc_content's cache size during codec
    pub fn cache_desc_content_size(&mut self, size: u16) -> &mut Self {
        assert!(self.desc_content_cached_size.is_none());
        self.desc_content_cached_size = Some(size);
        self
    }

    pub fn get_desc_content_cached_size(&self) -> u16 {
        self.desc_content_cached_size.unwrap()
    }

    // body_context
    pub fn body_context(&self) -> &NamedObjectBodyContext {
        &self.body_context
    }

    pub fn mut_body_context(&mut self) -> &mut NamedObjectBodyContext {
        &mut self.body_context
    }

    // inner
    fn has_flag(&self, flag_pos: u16) -> bool {
        (self.obj_flags & flag_pos) == flag_pos
    }
}

impl RawEncode for NamedObjectContext {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u16::raw_bytes().unwrap() * 2)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.obj_type.raw_encode(buf, purpose).map_err(|e| {
            log::error!("NamedObjectContext::raw_encode/obj_type error:{}", e);
            e
        })?;

        let buf = self.obj_flags.raw_encode(buf, purpose).map_err(|e| {
            log::error!("NamedObjectContext::raw_encode/obj_flags error:{}", e);
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for NamedObjectContext {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (obj_type, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!("NamedObjectContext::raw_decode/obj_type error:{}", e);
            e
        })?;

        let (obj_flags, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!("NamedObjectContext::raw_decode/obj_flags error:{}", e);
            e
        })?;

        Ok((NamedObjectContext::new(obj_type, obj_flags), buf))
    }
}

#[derive(Clone, Debug)]
pub struct ObjectBodyExt {
    // The object_id of the associated desc
    object_id: Option<ObjectId>,
}

impl Default for ObjectBodyExt {
    fn default() -> Self {
        Self { object_id: None }
    }
}

impl ObjectBodyExt {
    pub fn is_empty(&self) -> bool {
        self.object_id.is_none()
    }
}

// object body ext should use protobuf for codec
impl TryFrom<protos::ObjectBodyExt> for ObjectBodyExt {
    type Error = BuckyError;

    fn try_from(value: protos::ObjectBodyExt) -> BuckyResult<Self> {
        let mut ret = Self { object_id: None };

        if value.has_object_id() {
            ret.object_id = Some(ObjectId::clone_from_slice(value.get_object_id())?);
        }

        Ok(ret)
    }
}

impl TryFrom<&ObjectBodyExt> for protos::ObjectBodyExt {
    type Error = BuckyError;

    fn try_from(value: &ObjectBodyExt) -> BuckyResult<Self> {
        let mut ret = Self::new();

        if let Some(object_id) = &value.object_id {
            ret.set_object_id(object_id.to_vec()?);
        }

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(ObjectBodyExt);


#[derive(Clone)]
pub struct ObjectMutBody<B, O>
where
    O: ObjectType,
    B: BodyContent,
{
    prev_version: Option<HashValue>, // Perv versions's ObjectMutBody Hash
    update_time: u64, // Record the timestamp of the last update of body, in bucky time format
    content: B,       // Depending on the type, there can be different MutBody
    user_data: Option<Vec<u8>>, // Any data can be embedded. (e.g. json?)
    ext: Option<ObjectBodyExt>,
    obj_type: Option<PhantomData<O>>,
}

impl<B, O> std::fmt::Debug for ObjectMutBody<B, O>
where
    B: std::fmt::Debug + BodyContent,
    O: ObjectType,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObjectMutBody:{{ prev_version={:?}, update_time={}, version={}, format={}, content={:?}, ext={:?} user_data: ... }}",
            self.prev_version, self.update_time, self.content.version(), self.content.format(), self.content, self.ext,
        )
    }
}

#[derive(Clone)]
pub struct ObjectMutBodyBuilder<B, O>
where
    O: ObjectType,
    B: BodyContent,
{
    prev_version: Option<HashValue>,
    update_time: u64,
    content: B,
    user_data: Option<Vec<u8>>,
    ext: ObjectBodyExt,
    obj_type: Option<PhantomData<O>>,
}

impl<B, O> ObjectMutBodyBuilder<B, O>
where
    B: BodyContent,
    O: ObjectType,
{
    pub fn new(content: B) -> Self {
        Self {
            prev_version: None,
            update_time: bucky_time_now(),
            content,
            user_data: None,
            ext: ObjectBodyExt::default(),
            obj_type: None,
        }
    }

    pub fn update_time(mut self, value: u64) -> Self {
        self.update_time = value;
        self
    }

    pub fn option_update_time(mut self, value: Option<u64>) -> Self {
        if value.is_some() {
            self.update_time = value.unwrap();
        }

        self
    }

    pub fn prev_version(mut self, value: HashValue) -> Self {
        self.prev_version = Some(value);
        self
    }

    pub fn option_prev_version(mut self, value: Option<HashValue>) -> Self {
        self.prev_version = value;
        self
    }

    pub fn user_data(mut self, value: Vec<u8>) -> Self {
        self.user_data = Some(value);
        self
    }

    pub fn option_user_data(mut self, value: Option<Vec<u8>>) -> Self {
        self.user_data = value;
        self
    }

    pub fn object_id(mut self, value: ObjectId) -> Self {
        self.ext.object_id = Some(value);
        self
    }

    pub fn option_object_id(mut self, value: Option<ObjectId>) -> Self {
        self.ext.object_id = value;
        self
    }

    pub fn build(self) -> ObjectMutBody<B, O> {
        ObjectMutBody::<B, O> {
            prev_version: self.prev_version,
            update_time: self.update_time,
            content: self.content,
            user_data: self.user_data,
            obj_type: self.obj_type,
            ext: if self.ext.is_empty() {
                None
            } else {
                Some(self.ext)
            },
        }
    }
}

impl<B, O> ObjectMutBody<B, O>
where
    B: BodyContent,
    O: ObjectType,
{
    //pub fn new(content: B) -> ObjectMutBodyBuilder<B, O> {
    //    ObjectMutBodyBuilder::<B, O>::new(content)
    //}

    // real only methods

    pub fn prev_version(&self) -> &Option<HashValue> {
        &self.prev_version
    }

    pub fn update_time(&self) -> u64 {
        self.update_time
    }

    pub fn content(&self) -> &B {
        &self.content
    }

    pub fn into_content(self) -> B {
        self.content
    }

    pub fn user_data(&self) -> &Option<Vec<u8>> {
        &self.user_data
    }

    pub fn object_id(&self) -> &Option<ObjectId> {
        match &self.ext {
            Some(ext) => &ext.object_id,
            None => &None,
        }
    }

    pub fn verify_object_id(&self, object_id: &ObjectId) -> BuckyResult<()> {
        match &self.object_id() {
            Some(bind_object_id) => {
                if object_id != bind_object_id {
                    let msg = format!("object_id and object_id of body binding do not match! body object_id={}, object_id={}", bind_object_id, object_id);
                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::Unmatch, msg))
                } else {
                    Ok(())
                }
            }
            None => Ok(()),
        }
    }

    // Modify relate methods

    pub fn set_update_time(&mut self, value: u64) {
        self.update_time = value;
    }

    // Update the last modify time, and make sure it is greater than the old time
    pub fn increase_update_time(&mut self, mut value: u64) {
        if value < self.update_time {
            warn!(
                "object body new time is older than current time! now={}, cur={}",
                value, self.update_time
            );
            value = self.update_time + 1;
        }

        self.set_update_time(value);
    }

    pub fn content_mut(&mut self) -> &mut B {
        &mut self.content
    }

    pub fn set_userdata(&mut self, user_data: &[u8]) {
        let buf = Vec::from(user_data);
        self.user_data = Some(buf);
        self.increase_update_time(bucky_time_now());
    }

    pub fn set_object_id(&mut self, object_id: Option<ObjectId>) {
        if self.ext.is_none() {
            self.ext = Some(ObjectBodyExt::default());
        }

        self.ext.as_mut().unwrap().object_id = object_id;
    }

    /// Split the Body，Move everything inside
    /// ===
    /// * content: O::ContentType,
    /// * update_time: u64,
    /// * prev_version: Option<HashValue>,
    /// * user_data: Option<Vec<u8>>,
    pub fn split(self) -> (B, u64, Option<HashValue>, Option<Vec<u8>>) {
        let content = self.content;
        let update_time = self.update_time;
        let prev_version = self.prev_version;
        let user_data = self.user_data;
        (content, update_time, prev_version, user_data)
    }
}

impl<B, O> ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    pub fn calculate_hash(&self) -> BuckyResult<HashValue> {
        let hash = self.raw_hash_value().map_err(|e| {
            log::error!(
                "ObjectMutBody<B, O>::calculate_hash/raw_hash_value error:{}",
                e
            );
            e
        })?;

        Ok(hash)
    }
}

impl<'de, B, O> Default for ObjectMutBody<B, O>
where
    B: RawEncode + RawDecode<'de> + Default + BodyContent,
    O: ObjectType,
{
    fn default() -> Self {
        Self {
            prev_version: None,
            update_time: 0,
            content: B::default(),
            user_data: None,
            obj_type: None,
            ext: None,
        }
    }
}

impl<'de, B, O> RawEncodeWithContext<NamedObjectBodyContext> for ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectBodyContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        // body_flags
        let mut size = u8::raw_bytes().unwrap();

        // prev_version
        if self.prev_version.is_some() {
            size = size
                + self
                    .prev_version
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!("ObjectMutBody<B, O>::raw_measure/prev_version error:{}", e);
                        e
                    })?;
        }

        // update_time
        size += u64::raw_bytes().unwrap();

        // ext
        if let Some(ext) = &self.ext {
            if !ext.is_empty() {
                size = size
                    + u16::raw_bytes().unwrap()
                    + ext.raw_measure(purpose).map_err(|e| {
                        log::error!("ObjectMutBody<B, O>::raw_measure/ext error:{}", e);
                        e
                    })?;
            }
        }

        // verison+format
        size += u16::raw_bytes().unwrap();

        // content,包含usize+content
        let body_size = self.content.raw_measure(purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_measure/content error:{}", e);
            e
        })?;
        size += USize(body_size).raw_measure(purpose)?;
        size += body_size;

        // 缓存body_size
        ctx.cache_body_content_size(body_size);

        // user_data
        let ud = self.user_data.as_ref();
        let ud_size = if ud.is_some() {
            let ud = ud.unwrap();
            let len = ud.len();
            u64::raw_bytes().unwrap() + len
        } else {
            0usize
        };

        size = size + ud_size;

        Ok(size)
    }

    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectBodyContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        /*
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            let message = format!("[raw_encode] not enough buffer for ObjectMutBody， obj_type:{}, obj_type_code:{:?}", O::obj_type(), O::obj_type_code());
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, message));
        }
        */

        let ud = self.user_data.as_ref();

        // body_flags
        let mut body_flags = 0u8;

        if self.prev_version().is_some() {
            body_flags |= OBJECT_BODY_FLAG_PREV;
        }
        if ud.is_some() {
            body_flags |= OBJECT_BODY_FLAG_USER_DATA;
        }

        let mut encode_ext = false;
        if let Some(ext) = &self.ext {
            if !ext.is_empty() {
                body_flags |= OBJECT_BODY_FLAG_EXT;
                encode_ext = true;
            }
        }

        // Version information is added by default and no longer occupies the flags field
        let buf = body_flags.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_encode/body_flags error:{}", e);
            e
        })?;

        // prev_version
        let buf = if self.prev_version().is_some() {
            let buf = self
                .prev_version
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!("ObjectMutBody<B, O>::raw_encode/prev_version error:{}", e);
                    e
                })?;
            buf
        } else {
            buf
        };

        // update_time
        let buf = self.update_time.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_encode/update_time error:{}", e);
            e
        })?;

        // ext
        let buf = if encode_ext {
            let ext = self.ext.as_ref().unwrap();
            let size = ext.raw_measure(purpose)? as u16;
            let buf = size.raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/ext error:{}", e);
                e
            })?;

            let buf = ext.raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/ext error:{}", e);
                e
            })?;
            buf
        } else {
            buf
        };

        // version+format
        let buf = self
            .content
            .version()
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/version error:{}", e);
                e
            })?;
        let buf = self
            .content
            .format()
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/version error:{}", e);
                e
            })?;

        // content, include usize+content
        let buf = {
            let body_size = ctx.get_body_content_cached_size();

            let buf = USize(body_size).raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/content_usize error:{}", e);
                e
            })?;

            // 对bodycontent编码，采用精确大小的buf
            let body_buf = &mut buf[..body_size];
            let left_buf = self.content.raw_encode(body_buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/content error:{}", e);
                e
            })?;

            if left_buf.len() != 0 {
                warn!("encode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), body_size, left_buf.len());
                // assert!(left_buf.len() == 0);
            }

            &mut buf[body_size..]
        };

        // user_data
        let buf = if ud.is_some() {
            let ud = ud.unwrap();
            let len = ud.len();
            let buf = (len as u64).raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/user_data len error:{}", e);
                e
            })?;

            buf[..len].copy_from_slice(ud.as_slice());
            &mut buf[len..]
        } else {
            buf
        };

        Ok(buf)
    }
}

impl<'de, B, O> RawDecodeWithContext<'de, &NamedObjectBodyContext> for ObjectMutBody<B, O>
where
    B: RawDecode<'de> + BodyContent,
    O: ObjectType,
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        _ctx: &NamedObjectBodyContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        // body_flags
        let (body_flags, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/body_flags error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // prev_version
        let (prev_version, buf) = if (body_flags & OBJECT_BODY_FLAG_PREV) == OBJECT_BODY_FLAG_PREV {
            let (prev_version, buf) = HashValue::raw_decode(buf).map_err(|e| {
                let msg = format!("ObjectMutBody<B, O>::raw_decode/prev_version error:{}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;
            (Some(prev_version), buf)
        } else {
            (None, buf)
        };

        // update_time
        let (update_time, buf) = u64::raw_decode(buf).map_err(|e| {
            let msg = format!(
                "ObjectMutBody<B, O>::raw_decode/update_time error:{}, body={}",
                e,
                B::debug_info()
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // Here we try to read if there is an ext extension field, if it exists then we have to skip it for forward compatibility
        let mut ext = None;
        let buf = if (body_flags & OBJECT_BODY_FLAG_EXT) == OBJECT_BODY_FLAG_EXT {
            let (len, buf) = u16::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/ext error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;
            warn!(
                "read unknown body ext content! len={}, body={}",
                len,
                B::debug_info()
            );

            if len as usize > buf.len() {
                let msg = format!("read unknown body ext content but extend buffer limit, body={}, len={}, buf={}", B::debug_info(), len, buf.len());
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            if len > 0 {
                // Decode using exact size buffer
                let ext_buf = &buf[..len as usize];
                let (ret, _) = ObjectBodyExt::raw_decode(ext_buf).map_err(|e| {
                    let msg = format!(
                        "ObjectMutBody<B, O>::raw_decode/ext error:{}, body={}",
                        e,
                        B::debug_info()
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidData, msg)
                })?;

                ext = Some(ret);
            }
            
            // Skip the specified length
            &buf[len as usize..]
        } else {
            buf
        };

        // version
        let (version, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/version error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // format
        let (format, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/format error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // For BodyContent,we  use the decoding with option to be compatible with older versions of decoding
        let opt = RawDecodeOption { version, format };

        // body content
        let (content, buf) = {
            let (body_size, buf) = USize::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/content_usize error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            let body_size = body_size.value();
            if buf.len() < body_size {
                let msg = format!(
                    "invalid body content buffer size: expect={}, buf={}, body={}",
                    body_size,
                    buf.len(),
                    B::debug_info(),
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            // Decode using exact size buffer
            let body_buf = &buf[..body_size];
            let (content, left_buf) = B::raw_decode_with_option(&body_buf, &opt).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/content error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            if left_buf.len() != 0 {
                warn!("decode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), body_size, left_buf.len());
                // assert!(left_buf.len() == 0);
            }

            (content, &buf[body_size..])
        };

        // user_data
        let (user_data, buf) = if (body_flags & OBJECT_BODY_FLAG_USER_DATA)
            == OBJECT_BODY_FLAG_USER_DATA
        {
            let (len, buf) = u64::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/user_data len error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            let bytes = len as usize;
            if bytes > buf.len() {
                let msg = format!("ObjectMutBody<B, O>::raw_decode/user_data len overflow: body={}, len={}, left={}",
                        B::debug_info(),
                        len,
                        buf.len()
                    );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            let mut user_data = vec![0u8; bytes];
            user_data.copy_from_slice(&buf[..bytes]);
            (Some(user_data), &buf[bytes..])
        } else {
            (None, buf)
        };

        Ok((
            Self {
                prev_version,
                update_time,
                content,
                user_data,
                obj_type: None,
                ext,
            },
            buf,
        ))
    }
}

impl<'de, B, O> RawEncode for ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!();
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!();
    }

    fn raw_hash_encode(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectBodyContext::new();
        let size = self.raw_measure_with_context(&mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("ObjectMutBody<B, O>::rraw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code());
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = self.raw_encode_with_context(&mut buf, &mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("ObjectMutBody<B, O>::raw_encode/self.raw_encode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code());
            e
        })?;

        if left_buf.len() != 0 {
            warn!("encode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), size, left_buf.len());
            // assert!(left_buf.len() == 0);
        }

        Ok(buf)
    }
}

impl<'de, B, O> RawDecode<'de> for ObjectMutBody<B, O>
where
    B: RawDecode<'de> + BodyContent,
    O: ObjectType,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let ctx = NamedObjectBodyContext::new();
        Self::raw_decode_with_context(buf, &ctx)
    }
}

pub trait NamedObject<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    fn obj_flags(&self) -> u16;

    fn desc(&self) -> &O::DescType;

    fn desc_mut(&mut self) -> &mut O::DescType;

    fn body(&self) -> &Option<ObjectMutBody<O::ContentType, O>>;

    fn body_expect(&self, msg: &str) -> &ObjectMutBody<O::ContentType, O> {
        let msg = format!("expect obj_type {} body failed, msg:{}", O::obj_type(), msg);
        self.body().as_ref().expect(&msg)
    }

    fn body_mut(&mut self) -> &mut Option<ObjectMutBody<O::ContentType, O>>;

    fn body_mut_expect(&mut self, msg: &str) -> &mut ObjectMutBody<O::ContentType, O> {
        let msg = format!(
            "expect obj_type {} body_mut failed, msg:{}",
            O::obj_type(),
            msg
        );
        self.body_mut().as_mut().expect(&msg)
    }

    fn signs(&self) -> &ObjectSigns;

    fn signs_mut(&mut self) -> &mut ObjectSigns;

    fn nonce(&self) -> &Option<u128>;

    // Get the latest modification time of the body and sign sections, in bucky time
    fn latest_update_time(&self) -> u64 {
        let update_time = match self.body() {
            Some(body) => body.update_time(),
            None => 0_u64,
        };

        // If the signature time is relatively new, then take the signature time
        let latest_sign_time = self.signs().latest_sign_time();

        std::cmp::max(update_time, latest_sign_time)
    }
}

// TODO concat_idents! currently is an unstable util
#[macro_export]
macro_rules! declare_object {
    ($name:ident) => {
        type concat_idents!($name, Type) = cyfs_base::NamedObjType<concat_idents!($name, DescContent), concat_idents!($name, BodyContent)>;
        type concat_idents!($name, Builder) = cyfs_base::NamedObjectBuilder<concat_idents!($name, DescContent), concat_idents!($name, BodyContent)>;

        type concat_idents!($name, Id) = cyfs_base::NamedObjectId<concat_idents!($name, Type)>;
        type concat_idents!($name, Desc) = cyfs_base::NamedObjectDesc<concat_idents!($name, DescContent)>;
        type $name = cyfs_base::NamedObjectBase<concat_idents!($name,Type)>;
    }
}
