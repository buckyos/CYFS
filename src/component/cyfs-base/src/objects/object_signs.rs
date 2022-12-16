use crate::*;

#[derive(Clone, Debug)]
pub struct ObjectSigns {
    // 对Desc部分的签名，可以是多个，sign结构有的时候需要说明是“谁的签名”
    // 表示对Desc内容的认可。
    desc_signs: Option<Vec<Signature>>,

    // 对MutBody部分的签名，可以是多个。依赖MutBody的稳定编码
    body_signs: Option<Vec<Signature>>,
}

pub struct ObjectSignsHelper;

impl ObjectSignsHelper {
    pub fn set_sign(list: &mut Option<Vec<Signature>>, sign: Signature) {
        if list.is_none() {
            *list = Some(vec![]);
        }

        let list = list.as_mut().unwrap();
        list.clear();
        list.push(sign);
    }

    pub fn push_sign(list: &mut Option<Vec<Signature>>, sign: Signature) {
        if list.is_none() {
            *list = Some(vec![]);
        }

        let list = list.as_mut().unwrap();

        // 相同source的，只保留已有的签名，忽略时间和签名内容
        if let Some(cur) = list.iter_mut().find(|v| v.compare_source(&sign)) {
            if sign.sign_time() > cur.sign_time() {
                info!("desc sign with same source will update! new={:?}, cur={:?}", sign, cur);
                *cur = sign;
            } else{
                warn!("desc sign with same source already exists! sign={:?}", sign);
            }
        } else {
            list.push(sign);
        }
    }

    pub fn latest_sign_time(list: &Option<Vec<Signature>>) -> u64 {
        let mut ret = 0;
        if let Some(signs) = list.as_ref() {
            for sign in signs {
                if ret < sign.sign_time() {
                    ret = sign.sign_time();
                }
            }
        }
        ret
    }

    // FIXME dest和src合并，相同source签名是保留dest的，还是时间戳比较老的？
    pub fn merge(dest: &mut Option<Vec<Signature>>, src: &Option<Vec<Signature>>) -> usize {
        match src {
            Some(src) => {
                match dest {
                    Some(dest) => {
                        let mut ret = 0;
                        for src_item in src {
                            match dest.iter_mut().find(|s| s.compare_source(src_item)) {
                                Some(dest_item) => {
                                    if src_item.sign_time() > dest_item.sign_time() {
                                        *dest_item = src_item.clone();
                                    }
                                }
                                None => {
                                    dest.push(src_item.clone());
                                    ret += 1;
                                }

                            }
                        }
                        ret
                    }
                    None => {
                        // src里面的签名也需要确保去重
                        let mut ret = 0;
                        let mut dest_list: Vec<Signature> = Vec::new();
                        for src_item in src {
                            match dest_list.iter_mut().find(|s| s.compare_source(src_item)) {
                                Some(dest_item) => {
                                    if src_item.sign_time() > dest_item.sign_time() {
                                        *dest_item = src_item.clone();
                                    }
                                }
                                None => {
                                    dest_list.push(src_item.clone());
                                    ret += 1;
                                }
                            }
                        }

                        if !dest_list.is_empty() {
                            *dest = Some(dest_list);
                        }

                        ret
                    }
                }
            }
            None => 0,
        }
    }
}

#[derive(Clone)]
pub struct ObjectSignsBuilder {
    desc_signs: Option<Vec<Signature>>,
    body_signs: Option<Vec<Signature>>,
}

impl ObjectSignsBuilder {
    pub fn new() -> Self {
        Self {
            desc_signs: None,
            body_signs: None,
        }
    }

    pub fn set_desc_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::set_sign(&mut self.desc_signs, sign);
        self
    }

    pub fn set_body_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::set_sign(&mut self.body_signs, sign);
        self
    }

    pub fn push_desc_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::push_sign(&mut self.desc_signs, sign);
        self
    }

    pub fn push_body_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::push_sign(&mut self.body_signs, sign);
        self
    }

    pub fn build(self) -> ObjectSigns {
        ObjectSigns {
            desc_signs: self.desc_signs,
            body_signs: self.body_signs,
        }
    }
}

impl ObjectSigns {
    pub fn new() -> ObjectSignsBuilder {
        ObjectSignsBuilder::new()
    }

    pub fn is_desc_signs_empty(&self) -> bool {
        if let Some(signs) = &self.desc_signs {
            signs.len() == 0
        } else {
            true
        }
    }

    pub fn is_body_signs_empty(&self) -> bool {
        if let Some(signs) = &self.body_signs {
            signs.len() == 0
        } else {
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        self.is_desc_signs_empty() && self.is_body_signs_empty()
    }

    pub fn desc_signs(&self) -> Option<&Vec<Signature>> {
        self.desc_signs.as_ref()
    }

    pub fn body_signs(&self) -> Option<&Vec<Signature>> {
        self.body_signs.as_ref()
    }

    pub fn set_desc_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::set_sign(&mut self.desc_signs, sign)
    }

    pub fn set_body_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::set_sign(&mut self.body_signs, sign)
    }

    pub fn push_desc_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::push_sign(&mut self.desc_signs, sign)
    }

    pub fn push_body_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::push_sign(&mut self.body_signs, sign)
    }

    pub fn clear_desc_signs(&mut self) {
        self.desc_signs.take();
    }

    pub fn clear_body_signs(&mut self) {
        self.body_signs.take();
    }

    pub fn latest_body_sign_time(&self) -> u64 {
        ObjectSignsHelper::latest_sign_time(&self.body_signs)
    }

    pub fn latest_desc_sign_time(&self) -> u64 {
        ObjectSignsHelper::latest_sign_time(&self.desc_signs)
    }

    pub fn latest_sign_time(&self) -> u64 {
        std::cmp::max(self.latest_desc_sign_time(), self.latest_body_sign_time())
    }

    pub fn merge(&mut self, other: &ObjectSigns) -> usize {
        ObjectSignsHelper::merge(&mut self.desc_signs, &other.desc_signs)
            + ObjectSignsHelper::merge(&mut self.body_signs, &other.body_signs)
    }

    pub fn merge_ex(&mut self, other: &ObjectSigns, desc: bool, body: bool) -> usize {
        let mut ret = 0;
        if desc {
            ret += ObjectSignsHelper::merge(&mut self.desc_signs, &other.desc_signs);
        }
        if body {
            ret += ObjectSignsHelper::merge(&mut self.body_signs, &other.body_signs);
        }

        ret
    }
}

impl Default for ObjectSigns {
    fn default() -> Self {
        Self {
            desc_signs: None,
            body_signs: None,
        }
    }
}

impl RawEncodeWithContext<NamedObjectContext> for ObjectSigns {
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        let mut size = 0;

        if self.desc_signs.is_some() {
            ctx.with_desc_signs();
            assert!(ctx.has_desc_signs());
            size = size
                + self
                    .desc_signs
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "ObjectSigns::raw_measure_with_context/desc_signs error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.body_signs.is_some() {
            ctx.with_body_signs();
            assert!(ctx.has_body_signs());
            size = size
                + self
                    .body_signs
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "ObjectSigns::raw_measure_with_context/body_signs error:{}",
                            e
                        );
                        e
                    })?;
        }

        Ok(size)
    }

    // 调用之前，必须已经被正确measure过了
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        /*
        let size = self.raw_measure_with_context(ctx, purpose).unwrap();
        if buf.len() < size {
            let msg = format!(
                "not enough buffer for encode ObjectSigns, except={}, got={}",
                size,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        */

        let buf = if self.desc_signs.is_some() {
            assert!(ctx.has_desc_signs());
            self.desc_signs
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "ObjectSigns::raw_encode_with_context/desc_signs error:{}",
                        e
                    );
                    e
                })?
        } else {
            buf
        };

        let buf = if self.body_signs.is_some() {
            assert!(ctx.has_body_signs());
            self.body_signs
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "ObjectSigns::raw_encode_with_context/body_signs error:{}",
                        e
                    );
                    e
                })?
        } else {
            buf
        };

        Ok(buf)
    }
}

impl<'de> RawDecodeWithContext<'de, &NamedObjectContext> for ObjectSigns {
    fn raw_decode_with_context(
        buf: &'de [u8],
        ctx: &NamedObjectContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let (desc_signs, buf) = if ctx.has_desc_signs() {
            let (desc_signs, buf) = Vec::<Signature>::raw_decode(buf).map_err(|e| {
                log::error!(
                    "ObjectSigns::raw_decode_with_context/desc_signs error:{}",
                    e
                );
                e
            })?;
            (Some(desc_signs), buf)
        } else {
            (None, buf)
        };

        let (body_signs, buf) = if ctx.has_body_signs() {
            let (body_signs, buf) = Vec::<Signature>::raw_decode(buf).map_err(|e| {
                log::error!(
                    "ObjectSigns::raw_decode_with_context/body_signs error:{}",
                    e
                );
                e
            })?;
            (Some(body_signs), buf)
        } else {
            (None, buf)
        };

        Ok((
            Self {
                desc_signs,
                body_signs,
            },
            buf,
        ))
    }
}