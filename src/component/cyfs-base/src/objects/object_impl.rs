use crate::*;

use std::marker::PhantomData;

/// 子Desc类型系统
/// ===
/// * SubDescType: Sized + Sync + Send
/// * OwnerObj: SubDescType+Clone
/// * AreaObj: SubDescType+Clone
/// * AuthorObj: SubDescType+Clone
/// * PublicKeyObj: SubDescType+Clone
pub trait SubDescType: Sized + Sync + Send + Default {
    fn is_support() -> bool {
        true
    }
    fn is_none(&self) -> bool;
    fn is_some(&self) -> bool {
        !self.is_none()
    }
    fn inner_raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError>;
    fn inner_raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError>;
    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError>;
}

pub trait OwnerObj: SubDescType + Clone {
    type Inner;

    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self>;
    fn from_inner(inner: Self::Inner) -> Self;
}

pub trait AreaObj: SubDescType + Clone {
    type Inner;
    fn from_type_less(value: Option<Area>) -> BuckyResult<Self>;
    fn area_ref(&self) -> &Option<Area>;
    fn from_inner(inner: Self::Inner) -> Self;
}

pub trait AuthorObj: SubDescType + Clone {
    type Inner;
    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self>;
    fn from_inner(inner: Self::Inner) -> Self;
}

pub trait PublicKeyObj: SubDescType + Clone {
    fn from_type_less(single: Option<PublicKey>, mn: Option<MNPublicKey>) -> BuckyResult<Self>;
    fn has_single_key(&self) -> bool;
    fn has_mn_key(&self) -> bool;
}

/// 5 种 SubDescType
/// ====
/// * SubDescNone      表示不实现某个ObjectDesc
/// * Option<ObjectId> 用于OwnerObjectDesc或者AuthorObjectDesc
/// * Option<Area>     用于AreaObjectDesc
/// * PublicKey        用于SingleKeyObjectDesc
/// * MNPublicKey      用于MNKeyObjectDesc

/// SubDescNone
/// ===
/// 表示不实现某个 XXXObjectDesc
#[derive(Clone, Debug)]
pub struct SubDescNone();

impl Default for SubDescNone {
    fn default() -> Self {
        SubDescNone()
    }
}

impl SubDescType for SubDescNone {
    fn is_support() -> bool {
        false
    }

    fn is_none(&self) -> bool {
        true
    }

    fn inner_raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        panic!("Should not call here!!!");
    }

    fn inner_raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        panic!("Should not call here!!!");
    }

    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        Ok((SubDescNone(), buf))
    }
}

impl OwnerObj for SubDescNone {
    type Inner = SubDescNone;

    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self> {
        if value.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "owner objectdesc has been implement",
            ));
        }
        Ok(SubDescNone::default())
    }

    fn from_inner(inner: Self::Inner) -> Self {
        inner
    }
}

impl AreaObj for SubDescNone {
    type Inner = SubDescNone;

    fn from_type_less(value: Option<Area>) -> BuckyResult<Self> {
        if value.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "area objectdesc has been implement",
            ));
        }
        Ok(SubDescNone::default())
    }

    fn area_ref(&self) -> &Option<Area> {
        &None
    }

    fn from_inner(inner: Self::Inner) -> Self {
        inner
    }
}

impl AuthorObj for SubDescNone {
    type Inner = SubDescNone;

    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self> {
        if value.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "author objectdesc has been implement",
            ));
        }
        Ok(SubDescNone::default())
    }

    fn from_inner(inner: Self::Inner) -> Self {
        inner
    }
}

impl PublicKeyObj for SubDescNone {
    fn from_type_less(single: Option<PublicKey>, mn: Option<MNPublicKey>) -> BuckyResult<Self> {
        if single.is_some() || mn.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "public_key objectdesc has been implement",
            ));
        }
        Ok(SubDescNone::default())
    }

    fn has_single_key(&self) -> bool {
        false
    }

    fn has_mn_key(&self) -> bool {
        false
    }
}

/// Option<ObjectId>
/// ===
/// Option<ObjectId> 用于OwnerObjectDesc或者AuthorObjectDesc
impl SubDescType for Option<ObjectId> {
    fn is_none(&self) -> bool {
        self.is_none()
    }

    fn inner_raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        assert!(self.is_some());
        self.unwrap().raw_measure(purpose)
    }

    fn inner_raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        assert!(self.is_some());
        self.unwrap().raw_encode(buf, purpose)
    }

    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (id, buf) = ObjectId::raw_decode(buf).map_err(|e| {
            error!("Option<ObjectId>::inner_raw_decode/id error:{}", e);
            e
        })?;
        Ok((Some(id), buf))
    }
}

impl OwnerObj for Option<ObjectId> {
    type Inner = ObjectId;

    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self> {
        Ok(value)
    }

    fn from_inner(inner: Self::Inner) -> Self {
        Some(inner)
    }
}

impl AuthorObj for Option<ObjectId> {
    type Inner = ObjectId;

    fn from_type_less(value: Option<ObjectId>) -> BuckyResult<Self> {
        Ok(value)
    }

    fn from_inner(inner: Self::Inner) -> Self {
        Some(inner)
    }
}

impl OwnerObjectDesc for Option<ObjectId> {
    fn owner(&self) -> &Option<ObjectId> {
        self
    }
}

impl AuthorObjectDesc for Option<ObjectId> {
    fn author(&self) -> &Option<ObjectId> {
        self
    }
}

/// Option<ObjectId>
/// ===
/// Option<Area>     用于AreaObjectDesc
impl SubDescType for Option<Area> {
    fn is_none(&self) -> bool {
        self.is_none()
    }

    fn inner_raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        assert!(self.is_some());
        self.as_ref().unwrap().raw_measure(purpose)
    }

    fn inner_raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        assert!(self.is_some());
        self.as_ref().unwrap().raw_encode(buf, purpose)
    }

    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (area, buf) = Area::raw_decode(buf).map_err(|e| {
            error!("Option<Area>::inner_raw_decode/area error:{}", e);
            e
        })?;
        Ok((Some(area), buf))
    }
}

impl AreaObj for Option<Area> {
    type Inner = Area;

    fn from_type_less(value: Option<Area>) -> BuckyResult<Self> {
        Ok(value)
    }

    fn area_ref(&self) -> &Option<Area> {
        self
    }

    fn from_inner(inner: Self::Inner) -> Self {
        Some(inner)
    }
}

impl AreaObjectDesc for Option<Area> {
    fn area(&self) -> &Option<Area> {
        self
    }
}

/// PublicKey
/// ===
/// 用于SingleKeyObjectDesc
impl SubDescType for PublicKey {
    fn is_none(&self) -> bool {
        false
    }

    fn inner_raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        assert!(self.is_some());
        self.raw_measure(purpose)
    }

    fn inner_raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        assert!(self.is_some());
        self.raw_encode(buf, purpose)
    }

    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (key, buf) = PublicKey::raw_decode(buf).map_err(|e| {
            error!("PublicKey::inner_raw_decode/public_key error:{}", e);
            e
        })?;
        Ok((key, buf))
    }
}

impl PublicKeyObj for PublicKey {
    fn from_type_less(single: Option<PublicKey>, mn: Option<MNPublicKey>) -> BuckyResult<Self> {
        if mn.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "mn_public_key objectdesc has been implement",
            ));
        }

        if single.is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "single_public_key objectdesc has not been implement",
            ));
        }

        Ok(single.unwrap())
    }

    fn has_single_key(&self) -> bool {
        true
    }

    fn has_mn_key(&self) -> bool {
        false
    }
}

impl SingleKeyObjectDesc for PublicKey {
    fn public_key(&self) -> &PublicKey {
        self
    }
}

impl PublicKeyObjectDesc for PublicKey {
    fn public_key_ref(&self) -> Option<PublicKeyRef> {
        Some(self.into())
    }
}

/// MNPublicKey
/// ===
/// 用于MNKeyObjectDesc、

impl SubDescType for MNPublicKey {
    fn is_none(&self) -> bool {
        false
    }

    fn inner_raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        assert!(self.is_some());
        self.raw_measure(purpose)
    }

    fn inner_raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        assert!(self.is_some());
        self.raw_encode(buf, purpose)
    }

    fn inner_raw_decode<'de>(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (key, buf) = MNPublicKey::raw_decode(buf).map_err(|e| {
            error!("MNPublicKey::inner_raw_decode/mn_public_key error:{}", e);
            e
        })?;
        Ok((key, buf))
    }
}

impl PublicKeyObj for MNPublicKey {
    fn from_type_less(single: Option<PublicKey>, mn: Option<MNPublicKey>) -> BuckyResult<Self> {
        if single.is_some() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "single_public_key objectdesc has been implement",
            ));
        }

        if mn.is_none() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "mn_public_key objectdesc has not been implement",
            ));
        }

        Ok(mn.unwrap())
    }

    fn has_single_key(&self) -> bool {
        false
    }

    fn has_mn_key(&self) -> bool {
        true
    }
}

impl MNKeyObjectDesc for MNPublicKey {
    fn mn_public_key(&self) -> &MNPublicKey {
        self
    }
}

impl PublicKeyObjectDesc for MNPublicKey {
    fn public_key_ref(&self) -> Option<PublicKeyRef> {
        Some(self.into())
    }
}

/// DescContent
/// ====
/// Desc的用户自定义部分类型，默认是Typed
/// 如果是Buffer类型，则desc_content部分的size+buffer，都由desc_content自己encode/decode
#[derive(Eq, PartialEq)]
pub enum DescContentType {
    Typed,
    Buffer,
}

/// DescContent
/// ====
/// Desc的用户自定义部分接口定义
/// * 提供 fn obj_type() -> u16
/// * 如果想要实现解码的向前兼容，提供 fn version() -> u16
/// * 提供 OwnerType/AreaType/AuthorType/PublicKeyType 类型
///     * 指定 SubDescNone 表示不提供该特性
///     * 否则指定 其他四种 SubDescType, 参考上面 SubDescType 的说明
pub trait DescContent {
    fn obj_type() -> u16;

    fn obj_type_code() -> ObjectTypeCode {
        Self::obj_type().into()
    }

    fn desc_content_type() -> DescContentType {
        DescContentType::Typed
    }

    fn is_standard_object() -> bool {
        object_type_helper::is_standard_object(Self::obj_type())
    }

    fn is_core_object() -> bool {
        object_type_helper::is_core_object(Self::obj_type())
    }

    fn is_decapp_object() -> bool {
        object_type_helper::is_dec_app_object(Self::obj_type())
    }

    fn debug_info() -> String {
        String::from("DescContent")
    }

    // 如果想要实现解码的向前兼容，那么需要覆盖此方法
    fn version(&self) -> u8 {
        0
    }

    // 编码方式，如果想实现非Raw格式的编码，需要覆盖此方法
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_RAW
    }

    type OwnerType: OwnerObj;
    type AreaType: AreaObj;
    type AuthorType: AuthorObj;
    type PublicKeyType: PublicKeyObj;
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamedObjectDesc<T: DescContent> {
    // 基本部分 ObjectDesc
    obj_type: u16,
    dec_id: Option<ObjectId>,
    ref_objects: Option<Vec<ObjectLink>>,
    prev: Option<ObjectId>,
    create_timestamp: Option<HashValue>,
    create_time: Option<u64>,
    expired_time: Option<u64>,
    // 通过聚合的方式，保证
    // 1. NamedObjectDesc 严格控制这些 SubDescType 的二进制内存布局
    // 2. desc_content 不需要关心如何持有这些 SubDescType 的数据
    // 3. 如果这些 SubDescType 类型 实现了对应的 XXXObjectDesc,
    //    则 NamedObjectDesc 自动为他们实现 XXXObjectDesc
    owner: T::OwnerType,
    area: T::AreaType,
    author: T::AuthorType,
    public_key: T::PublicKeyType,

    // 扩展类型应该只关心自定义字段部分
    desc_content: T,
}

/// NamedObjectDesc<T> 的 ObjectDesc 自动实现
/// ===
/// 对任意 T: DescContent
/// 为 NamedObjectDesc<T> 实现 ObjectDesc
impl<T> ObjectDesc for NamedObjectDesc<T>
where
    T: DescContent + RawEncode,
    T::OwnerType: OwnerObj,
    T::AreaType: AreaObj,
    T::AuthorType: AuthorObj,
    T::PublicKeyType: PublicKeyObj,
{
    fn obj_type(&self) -> u16 {
        self.obj_type
    }

    fn calculate_id(&self) -> ObjectId {
        ObjectIdBuilder::new(self, self.obj_type_code())
            .area(self.area.area_ref().as_ref())
            .single_key(self.public_key.has_single_key())
            .mn_key(self.public_key.has_mn_key())
            .owner(self.owner.is_some())
            .build()
    }

    fn dec_id(&self) -> &Option<ObjectId> {
        &self.dec_id
    }

    fn ref_objs(&self) -> &Option<Vec<ObjectLink>> {
        &self.ref_objects
    }

    fn prev(&self) -> &Option<ObjectId> {
        &self.prev
    }

    fn create_timestamp(&self) -> &Option<HashValue> {
        &self.create_timestamp
    }

    fn create_time(&self) -> u64 {
        // 如果不存在，则返回0
        self.create_time.unwrap_or(0)
    }

    fn option_create_time(&self) -> Option<u64> {
        self.create_time
    }

    fn expired_time(&self) -> Option<u64> {
        self.expired_time
    }
}

/// NamedObjectDesc<T> 的 OwnerObjectDesc 自动实现
/// ===
/// 如果 T::OwnerType 实现了 OwnerObjectDesc
/// 则自动为 NamedObjectDesc<T> 实现 OwnerObjectDesc
impl<T> OwnerObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::OwnerType: OwnerObjectDesc,
{
    fn owner(&self) -> &Option<ObjectId> {
        self.owner.owner()
    }
}

impl OwnerObjectDesc for SubDescNone {
    fn owner(&self) -> &Option<ObjectId> {
        &None
    }
}

/// NamedObjectDesc<T> 的 AreaObjectDesc 自动实现
/// ===
/// 如果 T::AreaType 实现了 AreaObjectDesc
/// 则自动为 NamedObjectDesc<T> 实现 AreaObjectDesc
impl<T> AreaObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::AreaType: AreaObjectDesc,
{
    fn area(&self) -> &Option<Area> {
        self.area.area_ref()
    }
}

impl AreaObjectDesc for SubDescNone {
    fn area(&self) -> &Option<Area> {
        &None
    }
}

/// NamedObjectDesc<T> 的 AuthorObjectDesc 自动实现
/// ===
/// 如果 T::AuthorType 实现了 AuthorObjectDesc
/// 则自动为 NamedObjectDesc<T> 实现 AuthorObjectDesc
impl<T> AuthorObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::AuthorType: AuthorObjectDesc,
{
    fn author(&self) -> &Option<ObjectId> {
        self.author.author()
    }
}

impl AuthorObjectDesc for SubDescNone {
    fn author(&self) -> &Option<ObjectId> {
        &None
    }
}

/// NamedObjectDesc<T> 的 SingleKeyObjectDesc 自动实现
/// ===
/// 如果 T::PublicKeyType 实现了 SingleKeyObjectDesc+PublicKeyObjectDesc
/// 则自动实现 SingleKeyObjectDesc+PublicKeyObjectDesc
impl<T> SingleKeyObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::PublicKeyType: SingleKeyObjectDesc,
{
    fn public_key(&self) -> &PublicKey {
        self.public_key.public_key()
    }
}

/// NamedObjectDesc<T> 的 MNKeyObjectDesc 自动实现
/// ===
/// 如果 T::PublicKeyType 实现了 MNKeyObjectDesc+PublicKeyObjectDesc
/// 则自动实现了 MNKeyObjectDesc+PublicKeyObjectDesc
impl<T> MNKeyObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::PublicKeyType: MNKeyObjectDesc,
{
    fn mn_public_key(&self) -> &MNPublicKey {
        self.public_key.mn_public_key()
    }
}

/// NamedObjectDesc<T> 的 PublicKeyObjectDesc 自动实现
/// ===
/// SingleKeyObjectDesc 或者  MNKeyObjectDesc 一旦实现了就一定会实现 PublicKeyObjectDesc
impl<T> PublicKeyObjectDesc for NamedObjectDesc<T>
where
    T: DescContent,
    T::PublicKeyType: PublicKeyObjectDesc,
{
    fn public_key_ref(&self) -> Option<PublicKeyRef> {
        self.public_key.public_key_ref()
    }
}

impl PublicKeyObjectDesc for SubDescNone {
    fn public_key_ref(&self) -> Option<PublicKeyRef> {
        None
    }
}

/// NamedObjectDesc<T> 的 构造器
/// ===
/// 通过Builder模式创建BaseObjectDesc<T>
/// 可选部分通过Builder的方法动态注入
#[derive(Clone)]
pub struct NamedObjectDescBuilder<T: DescContent> {
    // ObjectDesc
    obj_type: u16,
    dec_id: Option<ObjectId>,
    ref_objects: Option<Vec<ObjectLink>>,
    prev: Option<ObjectId>,
    create_timestamp: Option<HashValue>,
    create_time: Option<u64>,
    expired_time: Option<u64>,

    // Owner/Area/Author/PublicKey
    owner: Option<T::OwnerType>,
    area: Option<T::AreaType>,
    author: Option<T::AuthorType>,
    public_key: Option<T::PublicKeyType>,

    version: u16,

    // DescContent
    desc_content: T,
    // obj
}

impl<T: DescContent> NamedObjectDescBuilder<T> {
    pub fn new(obj_type: u16, t: T) -> Self {
        Self {
            // ObjectDesc
            obj_type: obj_type,
            dec_id: None,
            ref_objects: None,
            prev: None,
            create_timestamp: None,
            create_time: Some(bucky_time_now()),
            expired_time: None,

            // Owner/Area/Author/PublicKey
            owner: Some(T::OwnerType::default()),
            area: Some(T::AreaType::default()),
            author: Some(T::AuthorType::default()),
            public_key: Some(T::PublicKeyType::default()),

            version: 0,

            // DescContent
            desc_content: t,
        }
    }

    pub fn desc_content(&self) -> &T {
        &self.desc_content
    }

    pub fn mut_desc_content(&mut self) -> &mut T {
        &mut self.desc_content
    }

    // ObjectDesc

    pub fn dec_id(mut self, value: ObjectId) -> Self {
        self.dec_id = Some(value);
        self
    }

    pub fn option_dec_id(mut self, value: Option<ObjectId>) -> Self {
        self.dec_id = value;
        self
    }

    pub fn ref_objects(mut self, value: Vec<ObjectLink>) -> Self {
        self.ref_objects = Some(value);
        self
    }

    pub fn option_ref_objects(mut self, value: Option<Vec<ObjectLink>>) -> Self {
        self.ref_objects = value;
        self
    }

    pub fn prev(mut self, value: ObjectId) -> Self {
        self.prev = Some(value);
        self
    }

    pub fn option_prev(mut self, value: Option<ObjectId>) -> Self {
        self.prev = value;
        self
    }

    pub fn create_timestamp(mut self, value: HashValue) -> Self {
        self.create_timestamp = Some(value);
        self
    }

    pub fn option_create_timestamp(mut self, value: Option<HashValue>) -> Self {
        self.create_timestamp = value;
        self
    }

    pub fn create_time(mut self, value: u64) -> Self {
        self.create_time = Some(value);
        self
    }

    pub fn option_create_time(mut self, value: Option<u64>) -> Self {
        self.create_time = value;
        self
    }

    pub fn expired_time(mut self, value: u64) -> Self {
        self.expired_time = Some(value);
        self
    }

    pub fn option_expired_time(mut self, value: Option<u64>) -> Self {
        self.expired_time = value;
        self
    }

    // Owner/Area/Author/PublicKey
    pub fn owner(mut self, value: <T::OwnerType as OwnerObj>::Inner) -> Self {
        self.owner = Some(T::OwnerType::from_inner(value));
        self
    }

    pub fn option_owner(mut self, value: T::OwnerType) -> Self {
        self.owner = Some(value);
        self
    }

    pub fn area(mut self, value: <T::AreaType as AreaObj>::Inner) -> Self {
        self.area = Some(T::AreaType::from_inner(value));
        self
    }

    pub fn option_area(mut self, value: T::AreaType) -> Self {
        self.area = Some(value);
        self
    }

    pub fn author(mut self, value: <T::AuthorType as AuthorObj>::Inner) -> Self {
        self.author = Some(T::AuthorType::from_inner(value));
        self
    }

    pub fn option_author(mut self, value: T::AuthorType) -> Self {
        self.author = Some(value);
        self
    }

    pub fn public_key(mut self, value: T::PublicKeyType) -> Self {
        self.public_key = Some(value);
        self
    }

    pub fn option_public_key(mut self, value: Option<T::PublicKeyType>) -> Self {
        self.public_key = value;
        self
    }

    pub fn version(mut self, version: u16) -> Self {
        self.version = version;
        self
    }

    // 构造对象
    pub fn build(self) -> NamedObjectDesc<T> {
        NamedObjectDesc::<T> {
            obj_type: self.obj_type,
            dec_id: self.dec_id,
            ref_objects: self.ref_objects,
            prev: self.prev,
            create_timestamp: self.create_timestamp,
            create_time: self.create_time,
            expired_time: self.expired_time,

            owner: self.owner.unwrap(),
            area: self.area.unwrap(),
            author: self.author.unwrap(),
            public_key: self.public_key.unwrap(),

            desc_content: self.desc_content,
        }
    }
}

/// NamedObjectDesc<T> 行为定义
/// ===
/// * new 返回构造器
impl<T: DescContent> NamedObjectDesc<T> {
    // 标准对象
    pub fn new(t: T) -> NamedObjectDescBuilder<T> {
        NamedObjectDescBuilder::<T>::new(T::obj_type(), t)
    }

    pub fn content(&self) -> &T {
        &self.desc_content
    }

    pub fn content_mut(&mut self) -> &mut T {
        &mut self.desc_content
    }

    pub fn into_content(self) -> T {
        self.desc_content
    }
}

/// NamedObjectDesc<T> 的 编码
/// ===
/// * ctx: NamedObjectContext 从上层 NamedObjectBase 里传入编码的上下文
/// * 通过 ctx 压缩Option字段的编码
impl<T> RawEncodeWithContext<NamedObjectContext> for NamedObjectDesc<T>
where
    T: DescContent + RawEncode,
{
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        let mut size = 0;

        //
        // ObjectDesc
        //

        if self.dec_id.is_some() {
            ctx.with_dec_id();
            size += self.dec_id.unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/dec_id error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.ref_objects.is_some() {
            ctx.with_ref_objects();
            size = size + self.ref_objects.as_ref().unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/ref_objects error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.prev.is_some() {
            ctx.with_prev();
            size = size + self.prev.unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/prev error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.create_timestamp.is_some() {
            ctx.with_create_timestamp();
            size = size + self.create_timestamp.unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/create_timestamp error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.create_time.is_some() {
            ctx.with_create_time();
            size = size + u64::raw_bytes().unwrap();
        }

        if self.expired_time.is_some() {
            ctx.with_expired_time();
            size = size + self.expired_time.unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/expired_time error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        //
        // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
        //

        if self.owner.is_some() {
            ctx.with_owner();
            size = size + self.owner.inner_raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/owner error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.area.is_some() {
            ctx.with_area();
            size = size + self.area.inner_raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/area error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }
        if self.author.is_some() {
            ctx.with_author();
            size = size + self.author.inner_raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/author error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.public_key.is_some() {
            ctx.with_public_key();
            size = size + u8::raw_bytes().unwrap();
            size = size + self.public_key.inner_raw_measure(purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_measure_with_context/public_key error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        // 新版本起默认带version+format, 16bit
        size += u16::raw_bytes().unwrap();

        // desc_content，包括一个(u16长度 + DescContent)内容
        let desc_content_usize = self.desc_content.raw_measure(purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_measure_with_context/desc_content error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;
        if desc_content_usize > u16::MAX as usize {
            let msg = format!(
                "desc content encode length extend max limit! len={}, max={}",
                desc_content_usize,
                u16::MAX
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        size += u16::raw_bytes().unwrap();
        size += desc_content_usize;

        // 缓存desc_content大小
        ctx.cache_desc_content_size(desc_content_usize as u16);

        Ok(size)
    }

    // encode之前，必须已经调用过raw_measure_with_context
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        /*
        let size = self.raw_measure_with_context(ctx, purpose).unwrap();
        if buf.len() < size {
            let message  = format!("[raw_encode] not enough buffer for NamedObjectDesc, obj_type:{}, obj_type_code:{:?}", T::obj_type(), T::obj_type_code());
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, message));
        }
        */

        // ObjectDesc

        let mut buf = buf;
        if self.dec_id.is_some() {
            buf = self.dec_id.unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/dec_id error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.ref_objects.is_some() {
            buf = self.ref_objects.as_ref().unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/ref_objects error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.prev.is_some() {
            buf = self.prev.unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/prev error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.create_timestamp.is_some() {
            buf = self.create_timestamp.unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/create_timestamp error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.create_time.is_some() {
            buf = self.create_time.unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/create_time error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.expired_time.is_some() {
            buf = self.expired_time.unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/expired_time error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc

        if self.owner.is_some() {
            buf = self.owner.inner_raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/owner error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.area.is_some() {
            buf = self.area.inner_raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/area error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.author.is_some() {
            buf = self.author.inner_raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/inner_raw_encode error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        if self.public_key.is_some() {
            let key_type: u8 = if self.public_key.has_single_key() {
                OBJECT_PUBLIC_KEY_SINGLE
            } else if self.public_key.has_mn_key() {
                OBJECT_PUBLIC_KEY_MN
            } else {
                OBJECT_PUBLIC_KEY_NONE
            };
            buf = key_type.raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/key_type error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;

            buf = self.public_key.inner_raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_encode_with_context/public_key error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;
        }

        // 编码version, 8bits
        buf = self.desc_content.version().raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode_with_context/version error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        // 编码format，8bits
        buf = self.desc_content.format().raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode_with_context/format error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        // desc_content
        // 使用缓存的size，必须之前先调用过measure
        let desc_content_size = ctx.get_desc_content_cached_size();
        let buf = desc_content_size.raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode_with_context/desc_content_size error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        let buf = self.desc_content.raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode_with_context/desc_content error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        Ok(buf)
    }
}

/// NamedObjectDesc<T> 独立编解码，需要包含obj_type+obj_flags信息
/// ===
/// * [1] ctx 部分包含obj_type, obj_flags 信息(前5bits为0，区别于NamedObject里的ctx.obj_flags)
/// * [2] 其余部分为desc本身的编码
/// 需要注意Desc部分独立编码，需要在头部编码正确的NamedObjectContext；如果在NamedObject内部整体编码，则由外层保证
impl<T> RawEncode for NamedObjectDesc<T>
where
    T: DescContent + RawEncode,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size = ctx.raw_measure(purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_measure/ctx error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?
        + self.raw_measure_with_context(&mut ctx, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_measure/raw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        Ok(size)
    }

    // 外部对Desc编码，尽量使用to_vec编码，会节省一次raw_measure操作
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);

        let size = self.raw_measure_with_context(&mut ctx, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/raw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        assert!(buf.len() >= size);

        let buf = ctx.raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/ctx.raw_encode error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        let buf = self.raw_encode_with_context(buf, &mut ctx, purpose).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/self.raw_encode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        Ok(buf)
    }

    fn raw_encode_to_buffer(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size = ctx.raw_measure(&None).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_measure/ctx error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })? + self.raw_measure_with_context(&mut ctx, &None).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_measure/raw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = ctx.raw_encode(&mut buf, &None).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/ctx.raw_encode error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        let left_buf = self.raw_encode_with_context(left_buf, &mut ctx, &None).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/self.raw_encode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;
        assert!(left_buf.len() == 0);

        Ok(buf)
    }

    fn raw_hash_encode(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size =  ctx.raw_measure(&Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_hash_encode/ctx.raw_measure error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })? + self.raw_measure_with_context(&mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_hash_encode/raw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = ctx.raw_encode(&mut buf, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_encode/ctx.raw_encode error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code());
            e
        })?;

        let left_buf = self.raw_encode_with_context(left_buf, &mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_hash_encode/raw_encode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;
        assert!(left_buf.len() == 0);

        // println!("hash_code: size={}, buf={:?}", size, buf);
        Ok(buf)
    }
}

impl<'de, T> RawDecode<'de> for NamedObjectDesc<T>
where
    T: DescContent + RawDecode<'de>,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ctx, buf) = NamedObjectContext::raw_decode(buf).map_err(|e| {
            error!(
                "NamedObjectDesc<T>::raw_decode/ctx error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                T::obj_type(),
                T::obj_type_code()
            );
            e
        })?;

        let (desc, buf) = NamedObjectDesc::<T>::raw_decode_with_context(buf, ctx).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_decode/self.raw_decode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
            e
        })?;

        Ok((desc, buf))
    }
}

/// NamedObjectDesc<T> 的 解码
/// ===
/// * ctx: NamedObjectContext 从上层 NamedObjectBase 里传入编码的上下文
/// * 通过 ctx 获取 Option 字段信息
impl<'de, T> RawDecodeWithContext<'de, NamedObjectContext> for NamedObjectDesc<T>
where
    T: DescContent + RawDecode<'de>,
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        ctx: NamedObjectContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let obj_type = ctx.obj_type();
        //
        // ObjectDesc
        //

        let (dec_id, buf) = if ctx.has_dec_id() {
            ObjectId::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/dec_id error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        let (ref_objects, buf) = if ctx.has_ref_objects() {
            Vec::<ObjectLink>::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/ref_objects error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        let (prev, buf) = if ctx.has_prev() {
            ObjectId::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/prev error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        let (create_timestamp, buf) = if ctx.has_create_time_stamp() {
            HashValue::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/create_timestamp error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        let (create_time, buf) = if ctx.has_create_time() {
            u64::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/create_time error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        let (expired_time, buf) = if ctx.has_expired_time() {
            u64::raw_decode(buf).map(|(v,buf)|{
                (Some(v), buf)
            }).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/expired_time error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (None, buf)
        };

        //
        // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
        //
        let (owner, buf) = if ctx.has_owner() {
            if !T::OwnerType::is_support() {
                let msg = format!("owner field not support for object type={}", obj_type);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            T::OwnerType::inner_raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/owner error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (T::OwnerType::default(), buf)
        };

        let (area, buf) = if ctx.has_area() {
            if !T::AreaType::is_support() {
                let msg = format!("area field not support for object type={}", obj_type);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            T::AreaType::inner_raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/area error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (T::AreaType::default(), buf)
        };

        let (author, buf) = if ctx.has_author() {
            if !T::AuthorType::is_support() {
                let msg = format!("author field not support for object type={}", obj_type);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            T::AuthorType::inner_raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/author error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (T::AuthorType::default(), buf)
        };

        let (public_key, buf) = if ctx.has_public_key() {
            if !T::PublicKeyType::is_support() {
                let msg = format!("public key field not support for object type={}", obj_type);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            let (_key_type, buf) = u8::raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode/_key_type error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;

            T::PublicKeyType::inner_raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode_with_context/T::PublicKeyType error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?
        } else {
            (T::PublicKeyType::default(), buf)
        };

        // 这里尝试读取是否存在ext扩展字段，如果存在那么为了向前兼容要跳过
        let buf = if ctx.has_ext() {
            let (len, buf) = u16::raw_decode(buf).map_err(|e| {
                error!(
                    "NamedObjectDesc<T>::raw_decode/ext error:{}, obj_type:{}, obj_type_code:{:?}",
                    e,
                    T::obj_type(),
                    T::obj_type_code()
                );
                e
            })?;

            // 向前兼容，不认识的扩展直接跳过
            warn!(
                "read unknown ext content! len={}, obj_type:{}, obj_type_code:{:?}",
                len,
                T::obj_type(),
                T::obj_type_code()
            );

            if len as usize > buf.len() {
                let msg = format!("read unknown body ext content but extend buffer limit, obj_type:{}, len={}, buf={}",
                T::obj_type(), len, buf.len());
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            &buf[len as usize..]
        } else {
            buf
        };

        // version
        let (version, buf) = u8::raw_decode(buf).map_err(|e| {
            error!(
                "NamedObjectDesc<T>::raw_decode/version error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                T::obj_type(),
                T::obj_type_code()
            );
            e
        })?;

        // format
        let (format, buf) = u8::raw_decode(buf).map_err(|e| {
            error!(
                "NamedObjectDesc<T>::raw_decode/format error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                T::obj_type(),
                T::obj_type_code()
            );
            e
        })?;

        let opt = RawDecodeOption { version, format };

        // desc_content
        let (desc_content, buf) = {
            // 首先解码长度
            let (desc_content_size, buf) = u16::raw_decode(buf).map_err(|e|{
                error!("NamedObjectDesc<T>::raw_decode/_desc_content_size error:{}, obj_type:{}, obj_type_code:{:?}", e, T::obj_type(), T::obj_type_code()); 
                e
            })?;

            let desc_content_size = desc_content_size as usize;
            if buf.len() < desc_content_size {
                let msg = format!(
                    "invalid desc content buffer size: expect={}, buf={}",
                    desc_content_size,
                    buf.len()
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            // 使用精确大小的buffer解码
            let desc_content_buf = &buf[..desc_content_size];
            let (desc_content, left_buf) = T::raw_decode_with_option(desc_content_buf, &opt).map_err(|e| {
                error!(
                    "NamedObjectDesc<T>::raw_decode/content error:{}, obj_type:{}, obj_type_code:{:?}",
                    e,
                    T::obj_type(),
                    T::obj_type_code()
                );
                e
            })?;

            if left_buf.len() != 0 {
                let msg = format!(
                    "decode desc content buffer but remaining buf is not empty: obj_type={}, desc_content_size={}, remaining={}",
                    T::obj_type(),
                    desc_content_size,
                    left_buf.len()
                );
                warn!("{}", msg);
            }

            (desc_content, &buf[desc_content_size..])
        };

        Ok((
            Self {
                // ObjectDesc
                obj_type,
                dec_id,
                ref_objects,
                prev,
                create_timestamp,
                create_time,
                expired_time,

                // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
                owner,
                area,
                author,
                public_key,

                // desc_content
                desc_content,
            },
            buf,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct NamedObjectBase<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    // type ObjectType = O::ObjectType
    desc: O::DescType,
    body: Option<ObjectMutBody<O::ContentType, O>>,
    signs: ObjectSigns,
    nonce: Option<u128>,
}

pub struct NamedObjectBaseBuilder<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    desc: O::DescType,
    body: Option<ObjectMutBody<O::ContentType, O>>,
    signs: ObjectSigns,
    nonce: Option<u128>,
}

impl<O> NamedObjectBaseBuilder<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    pub fn new(desc: O::DescType) -> Self {
        Self {
            desc: desc,
            body: None,
            signs: ObjectSigns::default(),
            nonce: None,
        }
    }

    pub fn body(mut self, value: ObjectMutBody<O::ContentType, O>) -> Self {
        self.body = Some(value);
        self
    }

    pub fn signs(mut self, value: ObjectSigns) -> Self {
        self.signs = value;
        self
    }

    pub fn nonce(mut self, value: u128) -> Self {
        self.nonce = Some(value);
        self
    }

    pub fn option_nonce(mut self, value: Option<u128>) -> Self {
        self.nonce = value;
        self
    }

    // body

    pub fn build(self) -> NamedObjectBase<O> {
        NamedObjectBase::<O> {
            desc: self.desc,
            body: self.body,
            signs: self.signs,
            nonce: self.nonce,
        }
    }
}

impl<O> NamedObjectBase<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    pub fn new_builder(desc: O::DescType) -> NamedObjectBaseBuilder<O> {
        NamedObjectBaseBuilder::<O>::new(desc)
    }

    pub fn default(desc: O::DescType) -> Self {
        Self {
            desc,
            body: None,
            signs: ObjectSigns::default(),
            nonce: None,
        }
    }

    pub fn new_desc(desc: O::DescType, signs: ObjectSigns, nonce: Option<u128>) -> Self {
        Self {
            desc,
            body: None,
            signs: signs,
            nonce: nonce,
        }
    }

    /// 大卸八块，映射成其他类型，Move语义
    /// ===
    /// * desc: O::DescType,
    /// * body: Option<ObjectMutBody<O::ContentType, O>>
    /// * signs: ObjectSigns,
    /// * nonce: Option<u128>,
    ///
    /// 可进一步对body.unwrap().split()
    pub fn split(
        self,
    ) -> (
        O::DescType,
        Option<ObjectMutBody<O::ContentType, O>>,
        ObjectSigns,
        Option<u128>,
    ) {
        let desc = self.desc;
        let body = self.body;
        let signs = self.signs;
        let nonce = self.nonce;

        (desc, body, signs, nonce)
    }

    pub fn into_desc(self) -> O::DescType {
        self.desc
    }

    pub fn into_body(self) -> Option<ObjectMutBody<O::ContentType, O>> {
        self.body
    }

    pub fn get_obj_update_time(&self) -> u64 {
        let update_time = match &self.body {
            None => 0_u64,
            Some(body) => body.update_time(),
        };

        // 如果签名时间比较新，那么取签名时间
        let latest_sign_time = self.signs.latest_sign_time();

        std::cmp::max(update_time, latest_sign_time)
    }
}

impl<O> NamedObject<O> for NamedObjectBase<O>
where
    O: ObjectType,
    O::DescType: RawEncodeWithContext<NamedObjectContext>,
    O::ContentType: RawEncode + BodyContent,
{
    fn obj_flags(&self) -> u16 {
        let mut ctx = NamedObjectContext::new(self.desc.obj_type(), 0);
        self.raw_measure_with_context(&mut ctx, &None).unwrap();

        // TODO 计算flags不需要完整的measure
        ctx.obj_flags()
    }

    fn desc(&self) -> &O::DescType {
        &self.desc
    }

    fn desc_mut(&mut self) -> &mut O::DescType {
        &mut self.desc
    }

    fn body(&self) -> &Option<ObjectMutBody<O::ContentType, O>> {
        &self.body
    }

    fn body_mut(&mut self) -> &mut Option<ObjectMutBody<O::ContentType, O>> {
        &mut self.body
    }

    fn signs(&self) -> &ObjectSigns {
        &self.signs
    }

    fn signs_mut(&mut self) -> &mut ObjectSigns {
        &mut self.signs
    }

    fn nonce(&self) -> &Option<u128> {
        &self.nonce
    }
}

impl<O> RawEncodeWithContext<NamedObjectContext> for NamedObjectBase<O>
where
    O: ObjectType,
    O::DescType: RawEncodeWithContext<NamedObjectContext>,
    O::ContentType: RawEncode + BodyContent,
{
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        let mut size = 0;
        // obj_type+obj_flags
        size += u32::raw_bytes().unwrap();

        // desc
        size += self.desc().raw_measure_with_context(ctx, purpose).map_err(|e|{
            error!("NamedObjectBase<O>::raw_measure_ex/desc error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
            e
        })?;

        // mut body
        if self.body().as_ref().is_some() {
            ctx.with_mut_body();
            size = size + self.body().as_ref().unwrap().raw_measure_with_context(ctx.mut_body_context(), purpose).map_err(|e|{
                error!("NamedObjectBase<O>::raw_measure_ex/body error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
                e
            })?;
        }

        // signs
        size = size + self.signs().raw_measure_with_context(ctx, purpose).map_err(|e|{
            error!("NamedObjectBase<O>::raw_measure_ex/signs error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
            e
        })?;

        // nonce
        if self.nonce().as_ref().is_some() {
            ctx.with_nonce();
            size = size + self.nonce().as_ref().unwrap().raw_measure(purpose).map_err(|e|{
                error!("NamedObjectBase<O>::raw_measure_ex/nonce error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
                e
            })?;
        }

        Ok(size)
    }

    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        // obj_type+obj_flags
        let buf = ctx.raw_encode(buf, purpose).map_err(|e|{
            error!("NamedObjectBase<O>::raw_encode/raw_measure_ex error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
            e
        })?;

        // desc
        let buf = self
            .desc()
            .raw_encode_with_context(buf, ctx, purpose)
            .unwrap();

        // mut_body
        let buf = if self.body.is_some() {
            self.body.as_ref().unwrap().raw_encode_with_context(buf, ctx.mut_body_context(), purpose).map_err(|e|{
                error!("NamedObjectBase<O>::raw_encode/body error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
                e
            })?
        } else {
            buf
        };

        // signs
        let buf = self.signs.raw_encode_with_context(buf, ctx, purpose).map_err(|e|{
            error!("NamedObjectBase<O>::raw_encode/signs error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
            e
        })?;

        // nonce
        let buf = if self.nonce.is_some() {
            self.nonce.as_ref().unwrap().raw_encode(buf, purpose).map_err(|e|{
                error!("NamedObjectBase<O>::raw_encode/nonce error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
                e
            })?
        } else {
            buf
        };

        Ok(buf)
    }
}

// 任何强类型 NamedObject<D> 都可以直接编码
impl<O> RawEncode for NamedObjectBase<O>
where
    O: ObjectType,
    O::DescType: RawEncodeWithContext<NamedObjectContext>,
    O::ContentType: RawEncode + BodyContent,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut ctx = NamedObjectContext::new(self.desc.obj_type(), 0);
        self.raw_measure_with_context(&mut ctx, purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut ctx = NamedObjectContext::new(self.desc.obj_type(), 0);
        let size = self.raw_measure_with_context(&mut ctx, purpose)?;

        if buf.len() < size {
            let message = format!("[raw_encode] not enough buffer for NamedObjectBase, obj_type:{}, obj_type_code:{:?}", O::obj_type(), O::obj_type_code());
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, message));
        }
        self.raw_encode_with_context(buf, &mut ctx, purpose)
    }

    // 这里对object整体编码做优化，减少一次measure
    fn raw_encode_to_buffer(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectContext::new(self.desc.obj_type(), 0);
        let size = self.raw_measure_with_context(&mut ctx, &None)?;

        let mut encode_buf = vec![0u8; size];

        let buf = self.raw_encode_with_context(&mut encode_buf, &mut ctx, &None)?;
        assert_eq!(buf.len(), 0);

        Ok(encode_buf)
    }
}

// 有了类型信息，可以确定具体的类型，对具体对类型信息解码
impl<'de, O> RawDecode<'de> for NamedObjectBase<O>
where
    O: ObjectType,
    O::DescType: RawDecodeWithContext<'de, NamedObjectContext>,
    O::ContentType: RawDecode<'de> + BodyContent,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        // obj_type+obj_flags
        let (ctx, buf) = NamedObjectContext::raw_decode(buf).map_err(|e| {
            error!(
                "NamedObjectBase<O>::raw_decode/ctx error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                O::obj_type(),
                O::obj_type_code()
            );
            e
        })?;

        let ctx_ref = &ctx;

        // 只有 TypelessObjectType 类型才不接受检查
        //println!("object type:{}, expected:{}", ctx.obj_type(), O::obj_type());
        if O::obj_type() != OBJECT_TYPE_ANY {
            if ctx.obj_type() != O::obj_type() {
                let msg = format!(
                    "obj_type_code not match! required={:?}, got={:?}",
                    O::obj_type(),
                    ctx.obj_type()
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
            }
        }

        // desc
        let (desc, buf) = O::DescType::raw_decode_with_context(buf, ctx.clone()).map_err(|e| {
            error!(
                "NamedObjectBase<O>::raw_decode/desc error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                O::obj_type(),
                O::obj_type_code()
            );
            e
        })?;

        // mut_body
        let (body, buf) = if ctx_ref.has_mut_body() {
            let (body, buf) = ObjectMutBody::<O::ContentType, O>::raw_decode_with_context(
                buf,
                ctx.body_context(),
            )
            .map_err(|e| {
                error!(
                    "NamedObjectBase<O>::raw_decode/body error:{}, obj_type:{}, obj_type_code:{:?}",
                    e,
                    O::obj_type(),
                    O::obj_type_code()
                );
                e
            })?;
            (Some(body), buf)
        } else {
            (None, buf)
        };

        // signs
        let (signs, buf) = ObjectSigns::raw_decode_with_context(buf, ctx_ref).map_err(|e| {
            error!(
                "NamedObjectBase<O>::raw_decode/signs error:{}, obj_type:{}, obj_type_code:{:?}",
                e,
                O::obj_type(),
                O::obj_type_code()
            );
            e
        })?;

        // nonce
        let (nonce, buf) = if ctx_ref.has_nonce() {
            let (nonce, buf) = u128::raw_decode(buf).map_err(|e|{
                error!("NamedObjectBase<O>::raw_decode/nonce error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code()); 
                e
            })?;
            (Some(nonce), buf)
        } else {
            (None, buf)
        };

        Ok((
            NamedObjectBase::<O> {
                desc,
                body,
                signs,
                nonce,
            },
            buf,
        ))
    }
}

// BodyContent的需要实现的部分
pub trait BodyContent {
    // 这里是使用静态函数还是成员函数?是不是也存在故意编码老版本格式的需求?
    fn version(&self) -> u8 {
        0
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_RAW
    }

    fn debug_info() -> String {
        String::from("BodyContent")
    }
}

/// NamedObject Type 泛型定义
/// ===
/// 基于 NamedObjectDesc
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct NamedObjType<DC, BC> {
    desc_content: Option<PhantomData<DC>>,
    body_content: Option<PhantomData<BC>>,
}

impl<DC, BC> ObjectType for NamedObjType<DC, BC>
where
    DC: RawEncode + DescContent + Sync + Send + Clone,
    BC: Sync + Send + Clone + RawEncode + BodyContent,
{
    fn obj_type_code() -> ObjectTypeCode {
        DC::obj_type_code()
    }

    fn obj_type() -> u16 {
        DC::obj_type()
    }

    type DescType = NamedObjectDesc<DC>;
    type ContentType = BC;
}

#[derive(Clone)]
pub struct NamedObjectBuilder<DC, BC>
where
    DC: RawEncode + DescContent + Sync + Send + Clone,
    BC: Sync + Send + Clone + RawEncode + BodyContent,
{
    desc_builder: NamedObjectDescBuilder<DC>,
    body_builder: ObjectMutBodyBuilder<BC, NamedObjType<DC, BC>>,
    signs_builder: ObjectSignsBuilder,
    nonce: Option<u128>,
}

impl<DC, BC> NamedObjectBuilder<DC, BC>
where
    DC: RawEncode + DescContent + Sync + Send + Clone,
    BC: Sync + Send + Clone + RawEncode + BodyContent,
{
    pub fn new(desc_content: DC, body_content: BC) -> Self {
        let desc_builder = NamedObjectDescBuilder::<DC>::new(DC::obj_type(), desc_content);

        let body_builder = ObjectMutBodyBuilder::<BC, NamedObjType<DC, BC>>::new(body_content);

        let signs_builder = ObjectSignsBuilder::new();

        Self {
            desc_builder,
            body_builder,
            signs_builder,
            nonce: None,
        }
    }

    pub fn desc_builder(&self) -> &NamedObjectDescBuilder<DC> {
        &self.desc_builder
    }

    pub fn mut_desc_builder(&mut self) -> &mut NamedObjectDescBuilder<DC> {
        &mut self.desc_builder
    }

    pub fn body_builder(&self) -> &ObjectMutBodyBuilder<BC, NamedObjType<DC, BC>> {
        &self.body_builder
    }

    // desc

    pub fn dec_id(mut self, dec_id: ObjectId) -> Self {
        self.desc_builder = self.desc_builder.dec_id(dec_id);
        self
    }

    pub fn option_dec_id(mut self, dec_id: Option<ObjectId>) -> Self {
        self.desc_builder = self.desc_builder.option_dec_id(dec_id);
        self
    }

    pub fn ref_objects(mut self, ref_objects: Vec<ObjectLink>) -> Self {
        self.desc_builder = self.desc_builder.ref_objects(ref_objects);
        self
    }

    pub fn option_ref_objects(mut self, ref_objects: Option<Vec<ObjectLink>>) -> Self {
        self.desc_builder = self.desc_builder.option_ref_objects(ref_objects);
        self
    }

    pub fn prev(mut self, prev: ObjectId) -> Self {
        self.desc_builder = self.desc_builder.prev(prev);
        self
    }

    pub fn option_prev(mut self, prev: Option<ObjectId>) -> Self {
        self.desc_builder = self.desc_builder.option_prev(prev);
        self
    }

    pub fn create_timestamp(mut self, create_timestamp: HashValue) -> Self {
        self.desc_builder = self.desc_builder.create_timestamp(create_timestamp);
        self
    }

    pub fn option_create_timestamp(mut self, create_timestamp: Option<HashValue>) -> Self {
        self.desc_builder = self.desc_builder.option_create_timestamp(create_timestamp);
        self
    }

    pub fn no_create_time(mut self) -> Self {
        self.desc_builder = self.desc_builder.option_create_time(None);
        self
    }

    pub fn create_time(mut self, create_time: u64) -> Self {
        self.desc_builder = self.desc_builder.create_time(create_time);
        self
    }

    // 传入None，表示自动取当前时间，传入Some(x)，表示设置为具体时间
    pub fn option_create_time(mut self, create_time: Option<u64>) -> Self {
        if let Some(time) = create_time {
            self.desc_builder = self.desc_builder.create_time(time);
        }
        self
    }

    pub fn expired_time(mut self, expired_time: u64) -> Self {
        self.desc_builder = self.desc_builder.expired_time(expired_time);
        self
    }

    pub fn option_expired_time(mut self, expired_time: Option<u64>) -> Self {
        self.desc_builder = self.desc_builder.option_expired_time(expired_time);
        self
    }

    // sub desc

    pub fn owner(mut self, owner: <DC::OwnerType as OwnerObj>::Inner) -> Self {
        self.desc_builder = self.desc_builder.owner(owner);
        self
    }

    pub fn option_owner(mut self, owner: DC::OwnerType) -> Self {
        self.desc_builder = self.desc_builder.option_owner(owner);
        self
    }

    pub fn area(mut self, area: <DC::AreaType as AreaObj>::Inner) -> Self {
        self.desc_builder = self.desc_builder.area(area);
        self
    }

    pub fn option_area(mut self, area: DC::AreaType) -> Self {
        self.desc_builder = self.desc_builder.option_area(area);
        self
    }

    pub fn author(mut self, author: <DC::AuthorType as AuthorObj>::Inner) -> Self {
        self.desc_builder = self.desc_builder.author(author);
        self
    }

    pub fn option_author(mut self, author: DC::AuthorType) -> Self {
        self.desc_builder = self.desc_builder.option_author(author);
        self
    }

    pub fn public_key(mut self, public_key: DC::PublicKeyType) -> Self {
        self.desc_builder = self.desc_builder.public_key(public_key);
        self
    }

    pub fn option_public_key(mut self, public_key: Option<DC::PublicKeyType>) -> Self {
        self.desc_builder = self.desc_builder.option_public_key(public_key);
        self
    }

    // body

    pub fn update_time(mut self, update_time: u64) -> Self {
        self.body_builder = self.body_builder.update_time(update_time);
        self
    }

    pub fn prev_version(mut self, prev_version: HashValue) -> Self {
        self.body_builder = self.body_builder.prev_version(prev_version);
        self
    }

    pub fn user_data(mut self, user_data: Vec<u8>) -> Self {
        self.body_builder = self.body_builder.user_data(user_data);
        self
    }

    // signs

    pub fn set_desc_sign(mut self, sign: Signature) -> Self {
        self.signs_builder = self.signs_builder.set_desc_sign(sign);
        self
    }

    pub fn set_body_sign(mut self, sign: Signature) -> Self {
        self.signs_builder = self.signs_builder.set_body_sign(sign);
        self
    }

    pub fn push_desc_sign(mut self, sign: Signature) -> Self {
        self.signs_builder = self.signs_builder.push_desc_sign(sign);
        self
    }

    pub fn push_body_sign(mut self, sign: Signature) -> Self {
        self.signs_builder = self.signs_builder.push_body_sign(sign);
        self
    }

    // nonce
    pub fn nonce(mut self, nonce: u128) -> Self {
        self.nonce = Some(nonce);
        self
    }

    pub fn build(self) -> NamedObjectBase<NamedObjType<DC, BC>> {
        let desc = self.desc_builder.build();
        let body = self.body_builder.build();
        let signs = self.signs_builder.build();

        NamedObjectBaseBuilder::new(desc)
            .signs(signs)
            .body(body)
            .option_nonce(self.nonce)
            .build()
    }
}
