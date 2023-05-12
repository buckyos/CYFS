use crate::*;

use std::convert::TryFrom;
use std::marker::PhantomData;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct TypelessObjectBodyContent {
    // body_content关联的version和format
    version: u8,
    format: u8,

    content_buf: Vec<u8>,
}

impl BodyContent for TypelessObjectBodyContent {
    fn version(&self) -> u8 {
        self.version
    }

    fn format(&self) -> u8 {
        self.format
    }
}

impl TypelessObjectBodyContent {
    fn version(&self) -> u8 {
        self.version
    }

    fn format(&self) -> u8 {
        self.format
    }

    pub fn data(&self) -> &[u8] {
        &self.content_buf
    }
}

impl RawEncode for TypelessObjectBodyContent {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let len = self.content_buf.len();

        Ok(len)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let len = self.content_buf.len();

        // 外层在编解码body_content时候，会保证此值
        assert!(buf.len() >= len);

        // Buffer
        unsafe {
            std::ptr::copy(self.content_buf.as_ptr(), buf.as_mut_ptr(), len);
        }
        let buf = &mut buf[len..];

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for TypelessObjectBodyContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let opt = RawDecodeOption::default();
        Self::raw_decode_with_option(buf, &opt)
    }

    fn raw_decode_with_option(
        buf: &'de [u8],
        opt: &RawDecodeOption,
    ) -> BuckyResult<(Self, &'de [u8])> {
        // 外部传入的buf长度就是body_content的真正长度
        let size = buf.len();

        // Buffer
        let mut content_buf = vec![0u8; size];
        unsafe {
            std::ptr::copy(buf.as_ptr(), content_buf.as_mut_ptr(), size);
        }
        let buf = &buf[size..];

        Ok((
            Self {
                version: opt.version,
                format: opt.format,
                content_buf,
            },
            buf,
        ))
    }
}

/// 无类型的Buffer对象，丢失了SubDesc和DescContent部分的类型信息
/// 实际上也是可以做到带组合类型信息，不过Owner x Area x Author x PublicKey 一共有 24 种组合类型
/// 不过，既然丢失了类型信息，只提供组合类型信息也只是完成了类型信息的一半，
/// 可以通过提供build的方式重建具体的带类型信息的NamedObject，通过调用者注入具体的类型信息完成完整的重构
#[derive(Clone, Debug)]
pub struct TypelessObjectDesc {
    // 基本部分 ObjectDesc
    obj_type: u16,
    dec_id: Option<ObjectId>,
    ref_objects: Option<Vec<ObjectLink>>,
    prev: Option<ObjectId>,
    create_timestamp: Option<HashValue>,
    create_time: Option<u64>,
    expired_time: Option<u64>,
    // 丢失了原始类型信息，但是可以获取
    owner: Option<ObjectId>,
    area: Option<Area>,
    author: Option<ObjectId>,
    single_public_key: Option<PublicKey>,
    mn_public_key: Option<MNPublicKey>,

    // desc_content对应的version和format
    version: u8,
    format: u8,

    desc_content_len: u16,
    desc_content_buf: Vec<u8>,
}

impl ObjectDesc for TypelessObjectDesc {
    fn obj_type(&self) -> u16 {
        self.obj_type
    }

    fn calculate_id(&self) -> ObjectId {
        ObjectIdBuilder::new(self, self.obj_type_code())
            .area(self.area.area_ref().as_ref())
            .single_key(self.single_public_key.is_some())
            .mn_key(self.mn_public_key.is_some())
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
        self.create_time.unwrap_or(0)
    }

    fn option_create_time(&self) -> Option<u64> {
        self.create_time
    }

    fn expired_time(&self) -> Option<u64> {
        self.expired_time
    }
}

/// 丢失了SubDesc类型信息，用成员方法的方式暴露是否含有这些数据
impl TypelessObjectDesc {
    pub fn owner(&self) -> &Option<ObjectId> {
        &self.owner
    }

    pub fn area(&self) -> &Option<Area> {
        &self.area
    }

    pub fn author(&self) -> &Option<ObjectId> {
        &self.author
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn format(&self) -> u8 {
        self.format
    }

    pub fn content(&self) -> &Vec<u8> {
        &self.desc_content_buf
    }

    pub fn convert_to<DescContentT>(self) -> BuckyResult<NamedObjectDesc<DescContentT>>
    where
        DescContentT: for<'de> RawDecode<'de> + DescContent + Sync + Send + Clone,
    {
        // 必须使用带version和format的decode解码
        let buf = &self.desc_content_buf;
        let opt = RawDecodeOption {
            version: self.version,
            format: self.format,
        };

        let (desc_content, _) = DescContentT::raw_decode_with_option(buf, &opt).map_err(|e| {
            log::error!("TypelessObjectDesc::convert_to/desc_content error:{}", e);
            e
        })?;

        let builder = NamedObjectDesc::<DescContentT>::new(desc_content);
        let basic_desc = builder
            .option_dec_id(self.dec_id)
            .option_ref_objects(self.ref_objects)
            .option_prev(self.prev)
            .option_create_timestamp(self.create_timestamp)
            .option_create_time(self.create_time)
            .option_expired_time(self.expired_time)
            .option_owner(
                DescContentT::OwnerType::from_type_less(self.owner).map_err(|e| {
                    log::error!("TypelessObjectDesc::convert_to/option_owner error:{}", e);
                    e
                })?,
            )
            .option_area(
                DescContentT::AreaType::from_type_less(self.area).map_err(|e| {
                    log::error!("TypelessObjectDesc::convert_to/option_area error:{}", e);
                    e
                })?,
            )
            .option_author(
                DescContentT::AuthorType::from_type_less(self.author).map_err(|e| {
                    log::error!("TypelessObjectDesc::convert_to/option_author error:{}", e);
                    e
                })?,
            )
            .public_key(
                DescContentT::PublicKeyType::from_type_less(
                    self.single_public_key,
                    self.mn_public_key,
                )
                .map_err(|e| {
                    log::error!("TypelessObjectDesc::convert_to/public_key error:{}", e);
                    e
                })?,
            )
            .build();

        Ok(basic_desc)
    }

    pub fn public_key(&self) -> Option<PublicKeyRef> {
        if self.single_public_key.is_some() {
            let key = self.single_public_key.as_ref().unwrap();
            Some(key.into())
        } else if self.mn_public_key.is_some() {
            let key = self.mn_public_key.as_ref().unwrap();
            Some(key.into())
        } else {
            None
        }
    }
}

/// TypelessObjectDesc 用于计算ID用的编码
/// ===
/// * [1] ctx 部分包含obj_type, obj_flags 信息(前5bits为0，区别于NamedObject里的ctx.obj_flags)
/// * [2] 其余部分为desc本身的编码
impl RawEncode for TypelessObjectDesc {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size = ctx.raw_measure(purpose).map_err(|e|{
            error!("TypelessObjectDesc::raw_measure/ctx.raw_measure error:{}, obj_type:{}", e, self.obj_type);
            e
        })?
        + self.raw_measure_with_context(&mut ctx, purpose).map_err(|e|{
            error!("TypelessObjectDesc::raw_measure/raw_measure_with_context error:{}, obj_type:{}", e, self.obj_type);
            e
        })?;

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);

        let size = self
            .raw_measure_with_context(&mut ctx, purpose)
            .map_err(|e| {
                error!(
                    "TypelessObjectDesc::raw_encode/raw_measure_with_context error:{}, obj_type:{}",
                    e, self.obj_type
                );
                e
            })?;

        assert!(buf.len() >= size);

        let buf = ctx.raw_encode(buf, purpose).map_err(|e| {
            error!(
                "TypelessObjectDesc::raw_encode/ctx.raw_encode error:{}, obj_type:{}",
                e, self.obj_type
            );
            e
        })?;

        let buf = self
            .raw_encode_with_context(buf, &mut ctx, purpose)
            .map_err(|e| {
                error!(
                    "TypelessObjectDesc::raw_encode/raw_encode_with_context error:{}, obj_type:{}",
                    e, self.obj_type
                );
                e
            })?;

        Ok(buf)
    }

    fn raw_encode_to_buffer(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size = ctx.raw_measure(&None).map_err(|e|{
            error!("TypelessObjectDesc::raw_measure/ctx error:{}, obj_type:{}", e, self.obj_type);
            e
        })? + self.raw_measure_with_context(&mut ctx, &None).map_err(|e|{
            error!("TypelessObjectDesc::raw_measure/raw_measure_with_context error:{}, obj_type:{}", e, self.obj_type); 
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = ctx.raw_encode(&mut buf, &None).map_err(|e| {
            error!(
                "TypelessObjectDesc::raw_encode/ctx.raw_encode error:{}, obj_type:{}",
                e, self.obj_type
            );
            e
        })?;

        let left_buf = self.raw_encode_with_context(left_buf, &mut ctx, &None).map_err(|e|{
            error!("TypelessObjectDesc::raw_encode/self.raw_encode_with_context error:{}, obj_type:{}", e, self.obj_type); 
            e
        })?;
        if left_buf.len() != 0 {
            warn!("encode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", self.obj_type, size, left_buf.len());
            // assert!(left_buf.len() == 0);
        }

        Ok(buf)
    }

    fn raw_hash_encode(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectContext::new(self.obj_type, 0);
        let size = ctx.raw_measure(&Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("TypelessObjectDesc::raw_hash_encode/ctx error:{}, obj_type:{}", e, self.obj_type);
            e
        })? + self.raw_measure_with_context(&mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_hash_encode/raw_measure_with_context error:{}, obj_type:{}", e, self.obj_type); 
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = ctx
            .raw_encode(&mut buf, &Some(RawEncodePurpose::Hash))
            .map_err(|e| {
                error!(
                    "TypelessObjectDesc::raw_hash_encode/ctx.raw_encode error:{}, obj_type:{}",
                    e, self.obj_type
                );
                e
            })?;

        let left_buf = self.raw_encode_with_context(left_buf, &mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("NamedObjectDesc<T>::raw_hash_encode/self.raw_encode_with_context error:{}, obj_type:{}", e, self.obj_type()); 
            e
        })?;
        if left_buf.len() != 0 {
            warn!("decode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", self.obj_type, size, left_buf.len());
            // assert!(left_buf.len() == 0);
        }

        Ok(buf)
    }
}

/// TypelessObjectDesc 编码
/// ===
/// * [1] ctx 部分来自NamedObject里的ctx
/// * [2] 其余部分为desc本身的编码
impl RawEncodeWithContext<NamedObjectContext> for TypelessObjectDesc {
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        // obj_type 和 obj_flags在NamedObject上层编解码，此处把相关信息通过ctx传递给上层
        let mut size = 0;

        //
        // ObjectDesc
        //

        if self.dec_id.is_some() {
            ctx.with_dec_id();
            size = size
                + self.dec_id.unwrap().raw_measure(purpose).map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_measure_with_context/dec_id error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.ref_objects.is_some() {
            ctx.with_ref_objects();
            size = size
                + self
                    .ref_objects
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/ref_objects error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.prev.is_some() {
            ctx.with_prev();
            size = size
                + self.prev.unwrap().raw_measure(purpose).map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_measure_with_context/prev error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.create_timestamp.is_some() {
            ctx.with_create_timestamp();
            size = size
                + self
                    .create_timestamp
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                        "TypelessObjectDesc::raw_measure_with_context/create_timestamp error:{}",
                        e
                    );
                        e
                    })?;
        }

        if self.create_time.is_some() {
            ctx.with_create_time();
            size = size + u64::raw_bytes().unwrap();
        }

        if self.expired_time.is_some() {
            ctx.with_expired_time();
            size = size
                + self
                    .expired_time
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/expired_time error:{}",
                            e
                        );
                        e
                    })?;
        }

        //
        // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
        //

        if self.owner.is_some() {
            ctx.with_owner();
            size = size
                + self
                    .owner
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/owner error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.area.is_some() {
            ctx.with_area();
            size = size
                + self
                    .area
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/area error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.author.is_some() {
            ctx.with_author();
            size = size
                + self
                    .author
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/author error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.single_public_key.is_some() {
            ctx.with_public_key();
            size = size + u8::raw_bytes().unwrap();
            size = size
                + self
                    .single_public_key
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                        "TypelessObjectDesc::raw_measure_with_context/single_public_key error:{}",
                        e
                    );
                        e
                    })?;
        } else if self.mn_public_key.is_some() {
            ctx.with_public_key();
            size = size + u8::raw_bytes().unwrap();
            size = size
                + self
                    .mn_public_key
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_measure_with_context/mn_public_key error:{}",
                            e
                        );
                        e
                    })?;
        }

        // version+formmat
        size += u16::raw_bytes().unwrap();

        // desc_content
        size += u16::raw_bytes().unwrap();
        size += self.desc_content_buf.len();

        Ok(size)
    }

    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        _ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        // ObjectDesc
        let mut buf = buf;
        if self.dec_id.is_some() {
            buf = self.dec_id.unwrap().raw_encode(buf, purpose).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/dec_id error:{}",
                    e
                );
                e
            })?;
        }

        if self.ref_objects.is_some() {
            buf = self
                .ref_objects
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/ref_objects error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.prev.is_some() {
            buf = self.prev.unwrap().raw_encode(buf, purpose).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/prev error:{}",
                    e
                );
                e
            })?;
        }

        if self.create_timestamp.is_some() {
            buf = self
                .create_timestamp
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/create_timestamp error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.create_time.is_some() {
            buf = self
                .create_time
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/create_time error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.expired_time.is_some() {
            buf = self
                .expired_time
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/expired_time error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.owner.is_some() {
            buf = self.owner.unwrap().raw_encode(buf, purpose).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/owner error:{}",
                    e
                );
                e
            })?;
        }

        if self.area.is_some() {
            buf = self
                .area
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/area error:{}",
                        e
                    );
                    e
                })?;
        }

        if self.author.is_some() {
            buf = self.author.unwrap().raw_encode(buf, purpose).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/author error:{}",
                    e
                );
                e
            })?;
        }

        if self.single_public_key.is_some() {
            buf = OBJECT_PUBLIC_KEY_SINGLE
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/key_flag_single error:{}",
                        e
                    );
                    e
                })?;

            buf = self
                .single_public_key
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/single_public_key error:{}",
                        e
                    );
                    e
                })?;
        } else if self.mn_public_key.is_some() {
            buf = OBJECT_PUBLIC_KEY_MN.raw_encode(buf, purpose).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/key_flag_mn error:{}",
                    e
                );
                e
            })?;

            buf = self
                .mn_public_key
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_encode_with_context/mn_public_key error:{}",
                        e
                    );
                    e
                })?;
        } else {
            //
        }

        // 编码version, 8bits
        buf = self.version().raw_encode(buf, purpose).map_err(|e| {
            error!(
                "TypelessObjectDesc::raw_encode_with_context/version error:{}, obj_type:{}",
                e,
                self.obj_type()
            );
            e
        })?;

        // 编码format, 8bits
        buf = self.format().raw_encode(buf, purpose).map_err(|e| {
            error!(
                "TypelessObjectDesc::raw_encode_with_context/format error:{}, obj_type:{}",
                e,
                self.obj_type()
            );
            e
        })?;

        // desc_content_len
        buf = self
            .desc_content_len
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_encode_with_context/desc_content_len error:{}",
                    e
                );
                e
            })?;

        // desc_content_buf
        unsafe {
            std::ptr::copy(
                self.desc_content_buf.as_ptr(),
                buf.as_mut_ptr(),
                self.desc_content_len as usize,
            );
        }
        let buf = &mut buf[self.desc_content_buf.len()..];

        Ok(buf)
    }
}

/// TypelessObjectDesc 解码
/// ===
/// * [1] ctx 部分来自NamedObject里的ctx
/// * [2] 其余部分为desc本身的解码
impl<'de> RawDecodeWithContext<'de, NamedObjectContext> for TypelessObjectDesc {
    fn raw_decode_with_context(
        buf: &'de [u8],
        ctx: NamedObjectContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let obj_type = ctx.obj_type();

        //
        // ObjectDesc
        //

        let (dec_id, buf) = if ctx.has_dec_id() {
            ObjectId::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/dec_id error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        let (ref_objects, buf) = if ctx.has_ref_objects() {
            Vec::<ObjectLink>::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/ref_objects error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        let (prev, buf) = if ctx.has_prev() {
            ObjectId::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/prev error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        let (create_timestamp, buf) = if ctx.has_create_time_stamp() {
            HashValue::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/create_timestamp error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        let (create_time, buf) = if ctx.has_create_time() {
            u64::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/create_time error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        let (expired_time, buf) = if ctx.has_expired_time() {
            u64::raw_decode(buf)
                .map(|(v, buf)| (Some(v), buf))
                .map_err(|e| {
                    log::error!(
                        "TypelessObjectDesc::raw_decode_with_context/expired_time error:{}",
                        e
                    );
                    e
                })?
        } else {
            (None, buf)
        };

        //
        // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
        //
        let (owner, buf) = if ctx.has_owner() {
            let (owner, buf) = ObjectId::raw_decode(buf).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_decode_with_context/owner error:{}",
                    e
                );
                e
            })?;
            (Some(owner), buf)
        } else {
            (None, buf)
        };

        let (area, buf) = if ctx.has_area() {
            let (area, buf) = Area::raw_decode(buf).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_decode_with_context/area error:{}",
                    e
                );
                e
            })?;
            (Some(area), buf)
        } else {
            (None, buf)
        };

        let (author, buf) = if ctx.has_author() {
            let (author, buf) = ObjectId::raw_decode(buf).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_decode_with_context/author error:{}",
                    e
                );
                e
            })?;
            (Some(author), buf)
        } else {
            (None, buf)
        };

        let (single_public_key, mn_public_key, buf) = if ctx.has_public_key() {
            let (key_type, buf) = u8::raw_decode(buf).map_err(|e| {
                log::error!(
                    "TypelessObjectDesc::raw_decode_with_context/key_type error:{}",
                    e
                );
                e
            })?;
            match key_type {
                OBJECT_PUBLIC_KEY_SINGLE => {
                    let (single_public_key, buf) = PublicKey::raw_decode(buf).map_err(|e|{
                        log::error!("TypelessObjectDesc::raw_decode_with_context/single_public_key error:{}", e); 
                        e
                    })?;
                    (Some(single_public_key), None, buf)
                }
                OBJECT_PUBLIC_KEY_MN => {
                    let (mn_public_key, buf) = MNPublicKey::raw_decode(buf).map_err(|e| {
                        log::error!(
                            "TypelessObjectDesc::raw_decode_with_context/mn_public_key error:{}",
                            e
                        );
                        e
                    })?;
                    (None, Some(mn_public_key), buf)
                }
                _ => {
                    panic!("should not come here");
                }
            }
        } else {
            (None, None, buf)
        };

        // 预留的扩展字段
        let buf = if ctx.has_ext() {
            let (len, buf) = u16::raw_decode(buf).map_err(|e| {
                error!(
                    "NamedObjectDesc<T>::raw_decode/ext error:{}, obj_type:{}",
                    e, obj_type,
                );
                e
            })?;
            warn!(
                "read unknown ext content! len={},  obj_type:{}",
                len, obj_type
            );

            if len as usize > buf.len() {
                let msg = format!(
                    "read unknown ext content but extend buffer limit, obj_type:{}, len={}, buf={}",
                    obj_type,
                    len,
                    buf.len()
                );
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
                "TypelessObjectDesc::raw_decode/version error:{}, obj_type:{}",
                e, obj_type
            );
            e
        })?;

        // format
        let (format, buf) = u8::raw_decode(buf).map_err(|e| {
            error!(
                "TypelessObjectDesc::raw_decode/format error:{}, obj_type:{}",
                e, obj_type,
            );
            e
        })?;

        // len+desc_content
        let (desc_content_len, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!(
                "TypelessObjectDesc::raw_decode_with_context/desc_content_len error:{}",
                e
            );
            e
        })?;
        let size: usize = desc_content_len as usize;

        if size > buf.len() {
            log::error!("TypelessObjectDesc::raw_decode_with_context/desc_content_len overflow");
            return Err(BuckyError::from(BuckyErrorCode::InvalidData));
        }

        // desc_content_buf
        let mut desc_content_buf = vec![0u8; size];
        unsafe {
            std::ptr::copy(
                buf.as_ptr(),
                desc_content_buf.as_mut_ptr(),
                desc_content_buf.len(),
            );
        }
        let buf = &buf[desc_content_buf.len()..];

        // panic!("2");
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
                single_public_key,
                mn_public_key,

                // version+format
                version,
                format,

                // desc_content
                desc_content_len,
                desc_content_buf,
            },
            buf,
        ))
    }
}

pub enum TypelessCatagory {
    Any = 0,
    Standard = 1,
    Core = 2,
    DECApp = 3,
}

pub trait TypeCatagoryMark: Clone {
    fn catagory() -> TypelessCatagory;
}

#[derive(Clone, Debug)]
pub struct TypelessObjectType<T: TypeCatagoryMark> {
    sub_type: Option<PhantomData<T>>,
}

impl<T: TypeCatagoryMark> ObjectType for TypelessObjectType<T> {
    fn obj_type_code() -> ObjectTypeCode {
        // panic!("should not come here");
        return ObjectTypeCode::Custom;
    }

    fn obj_type() -> u16 {
        OBJECT_TYPE_ANY
    }

    type DescType = TypelessObjectDesc;
    type ContentType = TypelessObjectBodyContent;
}

impl<T: TypeCatagoryMark> NamedObjectBase<TypelessObjectType<T>> {
    // 转成强类型对象
    // 运行时类型匹配检查
    fn convert_to<DescContentT, BodyContentT>(
        self,
    ) -> BuckyResult<NamedObjectBase<NamedObjType<DescContentT, BodyContentT>>>
    where
        DescContentT: for<'de> RawDecode<'de> + RawEncode + DescContent + Sync + Send + Clone,
        BodyContentT: for<'de> RawDecode<'de> + Sync + Send + Clone + RawEncode + BodyContent,
    {
        let (desc, body, signs, nonce) = self.split();

        // 运行时类型匹配检查
        assert!(
            DescContentT::obj_type() == desc.obj_type()
                && DescContentT::obj_type_code() == desc.obj_type_code()
        );
        if DescContentT::obj_type() != desc.obj_type()
            || DescContentT::obj_type_code() != desc.obj_type_code()
        {
            return Err(BuckyError::new(
                BuckyErrorCode::NotMatch,
                format!(
                    "obj_type is not match, require:{}, input:{}",
                    desc.obj_type(),
                    DescContentT::obj_type()
                ),
            ));
        }

        let typed_desc = desc.convert_to().map_err(|e| {
            log::error!(
                "NamedObjectBase<TypelessObjectType<T>>::convert_to/typed_desc error:{}",
                e
            );
            e
        })?;

        let has_body = body.is_some();
        if !has_body {
            let typed_obj = NamedObjectBase::<NamedObjType<DescContentT, BodyContentT>>::new_desc(
                typed_desc, signs, nonce,
            );
            Ok(typed_obj)
        } else {
            let (content, update_time, prev_version, user_data) = body.unwrap().split();
            let buf = content.data();

            // 这里需要使用解码出来的version+format进行二次解析
            let opt = RawDecodeOption {
                version: content.version,
                format: content.format,
            };
            let (typed_content, _) =
                BodyContentT::raw_decode_with_option(buf, &opt).map_err(|e| {
                    log::error!(
                        "NamedObjectBase<TypelessObjectType<T>>::convert_to/typed_content error:{}",
                        e
                    );
                    e
                })?;

            let body =
                ObjectMutBodyBuilder::<BodyContentT, NamedObjType<DescContentT, BodyContentT>>::new(
                    typed_content,
                )
                .update_time(update_time)
                .option_prev_version(prev_version)
                .option_user_data(user_data)
                .build();

            Ok(
                NamedObjectBase::<NamedObjType<DescContentT, BodyContentT>>::new_builder(
                    typed_desc,
                )
                .body(body)
                .signs(signs)
                .option_nonce(nonce)
                .build(),
            )
        }
    }

    // 脱壳，强转成另外一种类型
    // 运行时类型匹配检查
    fn convert_to_typeless<M: TypeCatagoryMark>(
        self,
    ) -> BuckyResult<NamedObjectBase<TypelessObjectType<M>>> {
        // 运行时类型匹配检查
        match M::catagory() {
            TypelessCatagory::Any => {
                // ignore
            }
            TypelessCatagory::Standard => {
                assert!(self.desc().is_standard_object());
                if !self.desc().is_standard_object() {
                    return Err(BuckyError::new(
                        BuckyErrorCode::NotMatch,
                        format!(
                            "obj_type is not standard object, obj_type:{}",
                            self.desc().obj_type()
                        ),
                    ));
                }
            }
            TypelessCatagory::Core => {
                assert!(self.desc().is_core_object());
                if !self.desc().is_core_object() {
                    return Err(BuckyError::new(
                        BuckyErrorCode::NotMatch,
                        format!(
                            "obj_type is not core object, obj_type:{}",
                            self.desc().obj_type()
                        ),
                    ));
                }
            }
            TypelessCatagory::DECApp => {
                assert!(self.desc().is_dec_app_object());
                if !self.desc().is_dec_app_object() {
                    return Err(BuckyError::new(
                        BuckyErrorCode::NotMatch,
                        format!(
                            "obj_type is not dec app object, obj_type:{}",
                            self.desc().obj_type()
                        ),
                    ));
                }
            }
        };

        let (desc, body, signs, nonce) = self.split();

        let has_body = body.is_some();
        if !has_body {
            Ok(NamedObjectBase::<TypelessObjectType<M>>::new_desc(
                desc, signs, nonce,
            ))
        } else {
            let (content, update_time, prev_version, user_data) = body.unwrap().split();

            let body =
                ObjectMutBodyBuilder::<TypelessObjectBodyContent, TypelessObjectType<M>>::new(
                    content,
                )
                .update_time(update_time)
                .option_prev_version(prev_version)
                .option_user_data(user_data)
                .build();

            Ok(NamedObjectBase::<TypelessObjectType<M>>::new_builder(desc)
                .body(body)
                .signs(signs)
                .option_nonce(nonce)
                .build())
        }
    }
}

/// 从 TypelessAnyObject/TypelessCoreObject/TypelessStandardObject/TypelessDECAppObject 转成 具体的强类型
impl<T, DescContentT, BodyContentT> TryFrom<NamedObjectBase<TypelessObjectType<T>>>
    for NamedObjectBase<NamedObjType<DescContentT, BodyContentT>>
where
    T: TypeCatagoryMark,
    DescContentT: for<'de> RawDecode<'de> + RawEncode + DescContent + Sync + Send + Clone,
    BodyContentT: for<'de> RawDecode<'de> + Sync + Send + Clone + RawEncode + BodyContent,
{
    type Error = BuckyError;
    fn try_from(value: NamedObjectBase<TypelessObjectType<T>>) -> Result<Self, Self::Error> {
        value.convert_to()
    }
}

/// 从 TypelessAnyObject 转成 AnyNamedObject
impl TryFrom<TypelessAnyObject> for AnyNamedObject {
    type Error = BuckyError;
    fn try_from(value: TypelessAnyObject) -> Result<Self, Self::Error> {
        let obj_type_code = value.desc().obj_type_code();
        let obj_type = value.desc().obj_type();
        match obj_type_code {
            ObjectTypeCode::Device => Ok(AnyNamedObject::Standard(StandardObject::Device(
                Device::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Device error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::People => Ok(AnyNamedObject::Standard(StandardObject::People(
                People::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/People error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Group => Ok(AnyNamedObject::Standard(StandardObject::Group(
                Group::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Org error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::AppGroup => Ok(AnyNamedObject::Standard(StandardObject::AppGroup(
                AppGroup::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/AppGroup error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::UnionAccount => Ok(AnyNamedObject::Standard(
                StandardObject::UnionAccount(UnionAccount::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/UnionAccount error:{}", e);
                    e
                })?),
            )),
            ObjectTypeCode::Chunk => {
                unreachable!();
            }
            ObjectTypeCode::File => Ok(AnyNamedObject::Standard(StandardObject::File(
                File::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/File error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Dir => Ok(AnyNamedObject::Standard(StandardObject::Dir(
                Dir::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Dir error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Diff => Ok(AnyNamedObject::Standard(StandardObject::Diff(
                Diff::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Diff error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::ProofOfService => Ok(AnyNamedObject::Standard(
                StandardObject::ProofOfService(ProofOfService::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/ProofOfService error:{}", e);
                    e
                })?),
            )),
            ObjectTypeCode::Tx => Ok(AnyNamedObject::Standard(StandardObject::Tx(
                Tx::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Tx error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Action => Ok(AnyNamedObject::Standard(StandardObject::Action(
                Action::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Action error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::ObjectMap => Ok(AnyNamedObject::Standard(StandardObject::ObjectMap(
                ObjectMap::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/ObjectMap error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Contract => Ok(AnyNamedObject::Standard(StandardObject::Contract(
                Contract::try_from(value).map_err(|e| {
                    log::error!("AnyNamedObject::try_from/Contract error:{}", e);
                    e
                })?,
            ))),
            ObjectTypeCode::Custom => {
                assert!(object_type_helper::is_custom_object(obj_type));

                if object_type_helper::is_core_object(obj_type) {
                    Ok(AnyNamedObject::Core(
                        value.convert_to_typeless::<CoreTypeMark>().map_err(|e| {
                            log::error!("AnyNamedObject::try_from/Core error:{}", e);
                            e
                        })?,
                    ))
                } else {
                    Ok(AnyNamedObject::DECApp(
                        value.convert_to_typeless::<DECAppTypeMark>().map_err(|e| {
                            log::error!("AnyNamedObject::try_from/DECApp error:{}", e);
                            e
                        })?,
                    ))
                }
            }
        }
    }
}

// TODO: 提供从具体的强类型转成 TypelessXXXObject的转换
// 直接通过 RawEncode/RawDecode 也可以，但是会多一些消耗，
// 通过split的话，只需要对desc_content/body_content部分做RawEncode, 所以还是有价值的

#[derive(Clone)]
pub struct AnyTypeMark {}

impl TypeCatagoryMark for AnyTypeMark {
    fn catagory() -> TypelessCatagory {
        TypelessCatagory::Any
    }
}

// #[derive(Clone)]
// pub struct StandardTypeMark{

// }

// impl TypeCatagoryMark for StandardTypeMark {
//     fn catagory()->TypelessCatagory{
//         TypelessCatagory::Standard
//     }
// }

#[derive(Clone, Debug)]
pub struct CoreTypeMark {}

impl TypeCatagoryMark for CoreTypeMark {
    fn catagory() -> TypelessCatagory {
        TypelessCatagory::Core
    }
}

#[derive(Clone, Debug)]
pub struct DECAppTypeMark {}

impl TypeCatagoryMark for DECAppTypeMark {
    fn catagory() -> TypelessCatagory {
        TypelessCatagory::DECApp
    }
}

pub type TypelessAnyObject = NamedObjectBase<TypelessObjectType<AnyTypeMark>>;
// pub type TypelessStandardObject = NamedObjectBase<TypelessObjectType<StandardTypeMark>>;
pub type TypelessCoreObject = NamedObjectBase<TypelessObjectType<CoreTypeMark>>;
pub type TypelessDECAppObject = NamedObjectBase<TypelessObjectType<DECAppTypeMark>>;

// -------------------------------
// 示例 扩展对象
//-------------------------------

#[derive(RawEncode, RawDecode, Clone)]
struct ExtDescContent {
    to: PeopleId,
}

impl DescContent for ExtDescContent {
    fn obj_type() -> u16 {
        17u16
    }
    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone)]
struct ExtBodyContent {}

impl BodyContent for ExtBodyContent {}

type ExtType = NamedObjType<ExtDescContent, ExtBodyContent>;
type ExtBuilder = NamedObjectBuilder<ExtDescContent, ExtBodyContent>;

type ExtDesc = NamedObjectDesc<ExtDescContent>;
type ExtId = NamedObjectId<ExtType>;
type Ext = NamedObjectBase<ExtType>;

trait ExtObjectDesc {
    fn ext_id(&self) -> ExtId;
}

trait ExtObject {
    fn new(owner: &PeopleId, author: &ObjectId, to: PeopleId) -> ExtBuilder;
    fn to(&self) -> &PeopleId;
}

impl ExtObjectDesc for ExtDesc {
    fn ext_id(&self) -> ExtId {
        ExtId::try_from(self.calculate_id()).unwrap()
    }
}

impl ExtObject for Ext {
    fn new(owner: &PeopleId, author: &ObjectId, to: PeopleId) -> ExtBuilder {
        let desc_content = ExtDescContent { to };

        let body_content = ExtBodyContent {};
        ExtBuilder::new(desc_content, body_content)
            .owner(owner.object_id().clone())
            .author(author.clone())
    }

    fn to(&self) -> &PeopleId {
        &self.desc().content().to
    }
}

#[cfg(test)]
mod test {
    use crate::{NamedObject, ObjectId, PeopleId, RawConvertTo, RawFrom};

    use super::{
        Ext, ExtBodyContent, ExtDescContent, ExtObject, ExtObjectDesc, ObjectDesc,
        TypelessCoreObject,
    };

    #[test]
    fn typeless() {
        let owner = PeopleId::default();
        let author = ObjectId::default();
        let to = PeopleId::default();
        let ext = Ext::new(&owner, &author, to).no_create_time().build();
        let buf = ext.to_vec().unwrap();
        println!("\n\n");

        let obj_decode_from_typed = Ext::clone_from_slice(&buf).unwrap();
        println!("\n\n");

        let type_less_ext = TypelessCoreObject::clone_from_slice(&buf).unwrap();
        println!("\n\n");

        let buf = type_less_ext.to_vec().unwrap();
        println!("\n\n");

        let type_less_ext_2 = TypelessCoreObject::clone_from_slice(&buf).unwrap();
        assert!(type_less_ext_2.desc().calculate_id() == type_less_ext.desc().calculate_id());
        println!("\n\n");

        //

        let obj_decode_from_typeless = Ext::clone_from_slice(&buf).unwrap();
        println!("\n\n");

        let id_decode_from_typed = obj_decode_from_typed.desc().ext_id();
        let id_decode_from_typeless = obj_decode_from_typeless.desc().ext_id();

        println!("\n\nid_decode_from_typed:{}\n\n", id_decode_from_typed);
        println!(
            "\n\nid_decode_from_typeless:{}\n\n",
            id_decode_from_typeless
        );

        assert!(id_decode_from_typed == id_decode_from_typeless);

        let obj_convert_from_typeless = type_less_ext
            .convert_to::<ExtDescContent, ExtBodyContent>()
            .unwrap();
        println!("\n\n");

        let id_convert_from_typeless = obj_convert_from_typeless.desc().ext_id();
        println!("\n\n");

        println!("\n\nid_decode_from_typed:{}\n\n", id_decode_from_typed);
        println!(
            "\n\nid_convert_from_typeless:{}\n\n",
            id_convert_from_typeless
        );

        assert!(id_decode_from_typed == id_convert_from_typeless);
    }
}
