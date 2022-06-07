use crate::*;

use std::any::Any;

//能静态确定编码后大小
pub trait RawFixedBytes {
    fn raw_bytes() -> Option<usize> {
        None
    }
    fn raw_max_bytes() -> Option<usize> {
        Self::raw_bytes()
    }
    fn raw_min_bytes() -> Option<usize> {
        Self::raw_bytes()
    }
}

/*
// TODO rust stable稳定版本支持模板偏特化后，再把raw_hash_code从RawEncode里面提取出来单独的trait
pub trait RawHashCode {
    fn raw_hash_encode(&self) -> BuckyResult<HashValue>;
}


impl<T: RawEncode> RawHashCode for T {
    fn raw_hash_encode(&self) -> BuckyResult<HashValue> {
        let size = self.raw_measure(&None)?;
        let mut buf = vec![0u8;size];

        let remain_buf = self.raw_encode(&mut buf, &None)?;
        let remain_len = remain_buf.len();
        let encoded_buf = &buf[..(buf.len() - remain_len)];

        let hash = hash_data(encoded_buf);

        let hash_slice = unsafe { &*(hash.as_slice().as_ptr() as *const [u8; 32]) };
        Ok(HashValue::from(hash_slice))
    }
}
*/

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RawEncodePurpose {
    // 默认值，为序列化而编码，需要是完整编码
    Serialize,

    // 为计算hash而编码
    Hash,
}

#[derive(Debug, Clone)]
pub struct RawDecodeOption {
    pub version: u8,
    pub format: u8,
}

impl Default for RawDecodeOption {
    fn default() -> Self {
        Self {
            version: 0,
            format: OBJECT_CONTENT_CODEC_FORMAT_RAW,
        }
    }
}

//编码
pub trait RawEncode {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize>;
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]>;
    fn raw_tail_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a [u8]> {
        let remain_buf = self.raw_encode(buf, purpose)?;
        let remain_len = remain_buf.len();
        Ok(&buf[..(buf.len() - remain_len)])
    }

    // 直接编码到buffer
    fn raw_encode_to_buffer(&self) -> BuckyResult<Vec<u8>> {
        let size = self.raw_measure(&None)?;
        let mut encode_buf = vec![0u8; size];

        let buf = self.raw_encode(&mut encode_buf, &None)?;
        assert_eq!(buf.len(), 0);

        Ok(encode_buf)
    }

    // 计算对象的hash
    fn raw_hash_value(&self) -> BuckyResult<HashValue> {
        let encoded_buf = self.raw_hash_encode()?;
        Ok(self.hash_buf(&encoded_buf))
    }

    fn hash_buf(&self, encoded_buf: &[u8]) -> HashValue {
        hash_data(encoded_buf)
    }

    // 默认hash编码实现，子类可以覆盖
    fn raw_hash_encode(&self) -> BuckyResult<Vec<u8>> {
        let size = self.raw_measure(&Some(RawEncodePurpose::Hash))?;
        let mut buf = vec![0u8; size];
        let remain_buf = self.raw_encode(&mut buf, &Some(RawEncodePurpose::Hash))?;
        assert!(remain_buf.len() == 0);

        Ok(buf)
    }
}

pub trait RawEncodeWithContext<Context> {
    fn raw_measure_with_context(
        &self,
        _: &mut Context,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize>;
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        _: &mut Context,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]>;
    fn raw_tail_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        context: &mut Context,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a [u8]> {
        let remain_buf = self.raw_encode_with_context(buf, context, purpose)?;
        let remain_len = remain_buf.len();
        Ok(&buf[..(buf.len() - remain_len)])
    }
}

//解码
pub trait RawDecode<'de>: Sized {
    // 不带opt的解码，默认一般实现此方法
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])>;

    // 带opt的解码，如果想使用版本等高级解码特性，需要实现此方法
    fn raw_decode_with_option(
        buf: &'de [u8],
        _opt: &RawDecodeOption,
    ) -> BuckyResult<(Self, &'de [u8])> {
        Self::raw_decode(buf)
    }

    /*
    fn raw_hash_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8], HashValue)> {
        let (v, next_buf) = Self::raw_decode(buf)?;
        let hash = hash_data(&buf[..(buf.len() - next_buf.len())]);
        let hash_slice = unsafe { &*(hash.as_slice().as_ptr() as *const [u8; 32]) };
        Ok((v, next_buf, HashValue::from(hash_slice)))
    }
    */
}

pub trait RawDecodeWithContext<'de, Context>: Sized {
    fn raw_decode_with_context(buf: &'de [u8], _: Context) -> BuckyResult<(Self, &'de [u8])>;

    /*
    fn raw_hash_decode_with_context(
        buf: &'de [u8],
        context: Context,
    ) -> BuckyResult<(Self, &'de [u8], HashValue)> {
        let (v, next_buf) = Self::raw_decode_with_context(buf, context)?;
        let hash = hash_data(&buf[..(buf.len() - next_buf.len())]);
        let hash_slice = unsafe { &*(hash.as_slice().as_ptr() as *const [u8; 32]) };
        Ok((v, next_buf, HashValue::from(hash_slice)))
    }
    */
}

pub trait RawMergable: Clone + Any {
    fn raw_merge_ok(&self, other: &Self) -> bool;
}

impl<T: RawEncode + Eq + Clone + Any> RawMergable for T {
    fn raw_merge_ok(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_hash() {
        let buf = "test_hash_buf".as_bytes();
        let hash = hash_data(buf);

        let hash_slice = unsafe { &*(hash.as_slice().as_ptr() as *const [u8; 32]) };
    
        let hash2 = HashValue::from(hash_slice);
        assert_eq!(hash, hash2);
    }
}