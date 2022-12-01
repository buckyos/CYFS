use crate::*;

use std::fmt;
use std::marker::PhantomData;

pub trait ObjectDesc {
    fn obj_type(&self) -> u16;

    // 默认实现，从obj_type 转 obj_type_code
    fn obj_type_code(&self) -> ObjectTypeCode {
        let t = self.obj_type();
        t.into()
    }

    fn is_stand_object(&self) -> bool {
        let c = self.obj_type_code();
        c != ObjectTypeCode::Custom
    }

    fn is_core_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom && (t >= OBJECT_TYPE_CORE_START && t <= OBJECT_TYPE_CORE_END)
    }

    fn is_dec_app_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom
            && (t >= OBJECT_TYPE_DECAPP_START && t <= OBJECT_TYPE_DECAPP_END)
    }

    fn object_id(&self) -> ObjectId {
        self.calculate_id()
    }
    // 计算 id
    fn calculate_id(&self) -> ObjectId;

    // 获取所属 DECApp 的 id
    fn dec_id(&self) -> &Option<ObjectId>;

    // 链接对象列表
    fn ref_objs(&self) -> &Option<Vec<ObjectLink>>;

    // 前一个版本号
    fn prev(&self) -> &Option<ObjectId>;

    // 创建时的 BTC Hash
    fn create_timestamp(&self) -> &Option<HashValue>;

    // 创建时间戳，如果不存在，则返回0
    fn create_time(&self) -> u64;

    // 过期时间戳
    fn expired_time(&self) -> Option<u64>;
}

/// 有权对象，可能是PublicKey::Single 或 PublicKey::MN
pub trait PublicKeyObjectDesc {
    fn public_key_ref(&self) -> Option<PublicKeyRef>;
}

/// 单公钥有权对象，明确用了PublicKey::Single类型
/// 实现了该Trait的对象一定同时实现了PublicKeyObjectDesc
pub trait SingleKeyObjectDesc: PublicKeyObjectDesc {
    fn public_key(&self) -> &PublicKey;
}

/// 多公钥有权对象，明确用了PublicKey::MN类型
/// 实现了该Trait的对象一定同时实现了PublicKeyObjectDesc
pub trait MNKeyObjectDesc: PublicKeyObjectDesc {
    fn mn_public_key(&self) -> &MNPublicKey;
}

/// 有主对象
pub trait OwnerObjectDesc {
    fn owner(&self) -> &Option<ObjectId>;
}

/// 有作者对象
pub trait AuthorObjectDesc {
    fn author(&self) -> &Option<ObjectId>;
}

/// 有区域对象
pub trait AreaObjectDesc {
    fn area(&self) -> &Option<Area>;
}

// obj_flags: u16
// ========
// * 前5个bits是用来指示编码状态，不计入hash计算。（计算时永远填0）
// * 剩下的11bits用来标识desc header
//
// 段编码标志位
// --------
// 0:  是否加密 crypto（现在未定义加密结构，一定填0)
// 1:  是否包含 mut_body
// 2:  是否包含 desc_signs
// 3:  是否包含 body_signs
// 4:  是否包含 nonce
//
// ObjectDesc编码标志位
// --------
// 5:  是否包含 dec_id
// 6:  是否包含 ref_objecs
// 7:  是否包含 prev
// 8:  是否包含 create_timestamp
// 9:  是否包含 create_time
// 10: 是否包含 expired_time
//
// OwnerObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc 标志位
// ---------
// 11: 是否包含 owner
// 12: 是否包含 area
// 13: 是否包含 author
// 14: 是否包含 public_key
// 15: 是否包含扩展字段
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

// 是否包含扩展字段，预留的非DescContent部分的扩展，包括一个u16长度+对应的content
pub const OBJECT_FLAG_EXT: u16 = 0x01 << 15;

// 左闭右闭 区间定义
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
// 是否包含扩展字段，格式和desc一致
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
        self.obj_type <= 16u16
    }

    pub fn is_core_object(&self) -> bool {
        self.obj_type >= OBJECT_TYPE_CORE_START && self.obj_type <= OBJECT_TYPE_CORE_END
    }

    pub fn is_decapp_object(&self) -> bool {
        self.obj_type >= OBJECT_TYPE_DECAPP_START && self.obj_type <= OBJECT_TYPE_DECAPP_END
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

    // desc_content的缓存大小
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

#[derive(Clone)]
pub struct ObjectMutBody<B, O>
where
    O: ObjectType,
    B: BodyContent,
{
    prev_version: Option<HashValue>, // 上个版本的MutBody Hash
    update_time: u64,                // 时间戳
    content: B,                      // 根据不同的类型，可以有不同的MutBody
    user_data: Option<Vec<u8>>,      // 可以嵌入任意数据。（比如json?)
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
            "ObjectMutBody:{{ prev_version:{:?}, update_time:{}, version={}, format={}, content:{:?}, user_data: ... }}",
            self.prev_version, self.update_time, self.content.version(), self.content.format(), self.content,
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

    pub fn build(self) -> ObjectMutBody<B, O> {
        ObjectMutBody::<B, O> {
            prev_version: self.prev_version,
            update_time: self.update_time,
            content: self.content,
            user_data: self.user_data,
            obj_type: self.obj_type,
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

    // 只读接口

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

    // 编辑接口

    pub fn set_update_time(&mut self, value: u64) {
        self.update_time = value;
    }

    // 更新时间，并且确保大于旧时间
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

    /// 拆解Body，Move出内部数据
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

        // 默认都添加版本信息，不再占用flags字段
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

        // content,包含usize+content
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

        // 这里尝试读取是否存在ext扩展字段，如果存在那么为了向前兼容要跳过
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

            // 跳过指定的长度
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

        // 对于BodyContent，使用带option的解码，用以兼容老版本的解码
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

            // 使用精确大小的buffer解码
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

    fn nonce(&self) -> &Option<Nonce>;

    // 获取body和sign部分的最新的修改时间
    fn latest_update_time(&self) -> u64 {
        let update_time = match self.body() {
            Some(body) => body.update_time(),
            None => 0_u64,
        };

        // 如果签名时间比较新，那么取签名时间
        let latest_sign_time = self.signs().latest_sign_time();

        std::cmp::max(update_time, latest_sign_time)
    }
}


// TODO concat_idents!目前是unstable功能
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