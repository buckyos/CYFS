use crate::*;

use generic_array::{ArrayLength, GenericArray};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::Range;

// u8
impl RawFixedBytes for u8 {
    fn raw_bytes() -> Option<usize> {
        Some(1)
    }
}

impl RawEncode for u8 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(1)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 1 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u8",
            ));
        }
        buf[0] = *self;

        Ok(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for u8 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 1 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u8",
            ));
        }
        Ok((buf[0], &buf[1..]))
    }
}

// bool
impl RawFixedBytes for bool {
    fn raw_bytes() -> Option<usize> {
        Some(1)
    }
}

impl RawEncode for bool {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(1)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 1 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u8",
            ));
        }
        if *self {
            buf[0] = 1;
        } else {
            buf[0] = 0;
        }

        Ok(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for bool {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 1 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u8",
            ));
        }
        if buf[0] == 0 {
            Ok((false, &buf[1..]))
        } else {
            Ok((true, &buf[1..]))
        }
    }
}

// u16
impl RawFixedBytes for u16 {
    fn raw_bytes() -> Option<usize> {
        Some(2)
    }
}

impl RawEncode for u16 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(2)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 2 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u16",
            ));
        }
        buf[..2].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[2..])
    }
}

impl<'de> RawDecode<'de> for u16 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 2 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u16",
            ));
        }
        let mut b = [0u8; 2];
        b.copy_from_slice(&buf[..2]);
        let v = u16::from_be_bytes(b);
        Ok((v, &buf[2..]))
    }
}

// u32
impl RawFixedBytes for u32 {
    fn raw_bytes() -> Option<usize> {
        Some(4)
    }
}

impl RawEncode for u32 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(4)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 4 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u32",
            ));
        }
        buf[..4].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[4..])
    }
}

impl<'de> RawDecode<'de> for u32 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 4 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u32",
            ));
        }
        let mut b = [0u8; 4];
        b.copy_from_slice(&buf[..4]);
        let v = u32::from_be_bytes(b);
        Ok((v, &buf[4..]))
    }
}

impl RawFixedBytes for i32 {
    fn raw_bytes() -> Option<usize> {
        Some(4)
    }
}

impl RawEncode for i32 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(4)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 4 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u32",
            ));
        }
        buf[..4].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[4..])
    }
}

impl<'de> RawDecode<'de> for i32 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 4 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u32",
            ));
        }
        let mut b = [0u8; 4];
        b.copy_from_slice(&buf[..4]);
        let v = i32::from_be_bytes(b);
        Ok((v, &buf[4..]))
    }
}

// i64
impl RawFixedBytes for i64 {
    fn raw_bytes() -> Option<usize> {
        Some(8)
    }
}

impl RawEncode for i64 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(8)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 8 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for i64",
            ));
        }
        buf[..8].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[8..])
    }
}

impl<'de> RawDecode<'de> for i64 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 8 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for i64",
            ));
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&buf[..8]);
        let v = i64::from_be_bytes(b);
        Ok((v, &buf[8..]))
    }
}

// u64
impl RawFixedBytes for u64 {
    fn raw_bytes() -> Option<usize> {
        Some(8)
    }
}

impl RawEncode for u64 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(8)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 8 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u64",
            ));
        }

        buf[..8].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[8..])
    }
}

impl<'de> RawDecode<'de> for u64 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 8 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u64",
            ));
        }

        let mut b = [0u8; 8];
        b.copy_from_slice(&buf[..8]);
        let v = u64::from_be_bytes(b);
        Ok((v, &buf[8..]))
    }
}

// u128
impl RawFixedBytes for u128 {
    fn raw_bytes() -> Option<usize> {
        Some(16)
    }
}

impl RawEncode for u128 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(16)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < 16 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u128",
            ));
        }

        buf[..16].copy_from_slice(&self.to_be_bytes());

        Ok(&mut buf[16..])
    }
}

impl<'de> RawDecode<'de> for u128 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < 16 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for u128",
            ));
        }

        let mut b = [0u8; 16];
        b.copy_from_slice(&buf[..16]);
        let v = u128::from_be_bytes(b);
        Ok((v, &buf[16..]))
    }
}

const U6_MAX: u64 = (u8::MAX >> 2) as u64;
const U14_MAX: u64 = (u16::MAX >> 2) as u64;
const U30_MAX: u64 = (u32::MAX >> 2) as u64;
const VARSIZE_SUB_2_MAX: u64 = (u64::MAX >> 2) as u64;

struct VarSizeHelper;

impl VarSizeHelper {
    fn raw_measure(len: u64, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes;

        if len <= U6_MAX {
            bytes = 1;
        } else if len <= U14_MAX {
            bytes = 2;
        } else if len <= U30_MAX {
            bytes = 4;
        } else if len <= VARSIZE_SUB_2_MAX {
            bytes = 8;
        } else {
            let msg = format!("not enough buffer for size larger then {}, value={}", VARSIZE_SUB_2_MAX, len);
            error!("{}", msg);
            return Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                msg,
            ));
        }

        Ok(bytes)
    }

    fn raw_encode<'a>(
        len: u64,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut buf = buf;
        if len <= U6_MAX {
            let len = len as u8;
            let flag = len | 0b_00_000000;
            buf = flag.raw_encode(buf, purpose)?;
        } else if len <= U14_MAX {
            let len = len as u16;
            let flag = len | 0b_01_000000_00000000;
            buf = flag.raw_encode(buf, purpose)?;
        } else if len <= U30_MAX {
            let len = len as u32;
            let flag = len | 0b_11_000000_00000000_00000000_00000000;
            buf = flag.raw_encode(buf, purpose)?;
        } else if len <= VARSIZE_SUB_2_MAX {
            let len = len as u64;
            let flag =
                len | 0b_10_000000_00000000_00000000_00000000_00000000_00000000_00000000_00000000;
            buf = flag.raw_encode(buf, purpose)?;
        } else {
            return Err(BuckyError::new(
                BuckyErrorCode::NotSupport,
                "not enough buffer for size large then u64",
            ));
        }

        Ok(buf)
    }

    fn raw_decode(buf: &[u8]) -> BuckyResult<(u64, &[u8])> {
        let (first_byte, _buf) = u8::raw_decode(buf)?;

        let (len, buf) = if first_byte & 0b_11_000000 == 0b_00_000000 {
            let len = first_byte as u64;
            (len, _buf)
        } else if first_byte & 0b_11_000000 == 0b_01_000000 {
            let (value, buf) = u16::raw_decode(buf)?;
            let len = (value & 0b_00_111111_11111111) as u64;
            (len, buf)
        } else if first_byte & 0b_11_000000 == 0b_11_000000 {
            let (value, buf) = u32::raw_decode(buf)?;
            let len = (value & 0b_00_111111_11111111_11111111_11111111) as u64;
            (len, buf)
        } else if first_byte & 0b_11_000000 == 0b_10_000000 {
            let (value, buf) = u64::raw_decode(buf)?;
            let len = (value
                & 0b_00_111111_11111111_11111111_11111111_11111111_11111111_11111111_11111111)
                as u64;
            (len, buf)
        } else {
            panic!("invalid first byte: {}", first_byte);
        };

        Ok((len, buf))
    }
}

// 可变长度size，包括VarSize和USize两种基础类型
// USize最大支持的size范围是[0, usize::MAX>>2]，会根据size的实际大小，占用1-8个bytes，其中usize会根据32bit还是64bit不同而不同
//      在需要表示一些容器的大小时候使用此值，比如vec,map等
//      如果要同时支持32bit和64bit平台，那么要仔细考虑这个差异，可能会导致32bit平台无法解码64bit平台的一些结构(会被截断)
// BuckySize最大支持的size范围是[0, u64::MAX>>2]，会根据size的实际大小，占用1-8个bytes，在需要明确的size情况下使用该结构体

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BuckySize(pub u64);

impl BuckySize {
    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Deref for BuckySize {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BuckySize {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RawEncode for BuckySize {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        VarSizeHelper::raw_measure(self.0, purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        VarSizeHelper::raw_encode(self.0, buf, purpose)
    }
}

impl<'de> RawDecode<'de> for BuckySize {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (len, buf) = VarSizeHelper::raw_decode(buf)?;

        Ok((BuckySize(len), buf))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct USize(pub usize);

impl USize {
    pub fn value(&self) -> usize {
        self.0
    }
}

impl Deref for USize {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for USize {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RawEncode for USize {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        VarSizeHelper::raw_measure(self.0 as u64, purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        VarSizeHelper::raw_encode(self.0 as u64, buf, purpose)
    }
}

impl<'de> RawDecode<'de> for USize {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (len, buf) = VarSizeHelper::raw_decode(buf)?;

        if len > usize::MAX as u64 {
            let msg = format!(
                "len extend usize max size on 32bit platform! len={}, usize::MAX={}",
                len,
                usize::MAX
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        Ok((USize(len as usize), buf))
    }
}

#[test]
fn usize_x() {
    println!("\ntest {}", 63);
    {
        let len = 63u8;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 64);
    {
        let len = 64u8;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 16383u16);
    {
        let len = 16383u16;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 16384u16);
    {
        let len = 16384u16;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 1073741823u32);
    {
        let len = 1073741823u32;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 1073741824u32);
    {
        let len = 1073741824u32;
        let ulen = USize(len as usize);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = USize::raw_decode(&buf).unwrap();
        assert_eq!(len as usize, decode_ulen.value());
    }

    println!("\ntest {}", 4611686018427387902u64);
    {
        let len = 4611686018427387902u64;
        let ulen = BuckySize(len);
        let size = ulen.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; size];
        let _rest_buf = ulen.raw_encode(&mut buf, &None).unwrap();
        let (decode_ulen, _decode_buf) = BuckySize::raw_decode(&buf).unwrap();
        assert_eq!(len, decode_ulen.value());
    }

    println!("\ntest {}", 4611686018427387903u64);
    {
        let len = 4611686018427387903u64;
        let ulen = USize(len as usize);

        #[cfg(target_pointer_width = "32")]
        assert!(!ulen.raw_measure(&None).is_err());

        #[cfg(target_pointer_width = "64")]
        assert!(!ulen.raw_measure(&None).is_err());
    }
}

// [T]
impl<T: RawEncode> RawFixedBytes for [T] {
    fn raw_min_bytes() -> Option<usize> {
        Some(1)
    }
}

impl<T: RawEncode> RawEncode for [T] {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.len());
        let mut bytes = ulen.raw_measure(purpose).unwrap();
        for e in self {
            bytes += e.raw_measure(purpose)?;
        }
        Ok(bytes)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < Self::raw_min_bytes().unwrap() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for vec",
            ));
        }

        let ulen = USize(self.len());
        let buf = ulen.raw_encode(buf, purpose)?;
        let mut offset: usize = 0;
        for e in self {
            offset = buf.len() - e.raw_encode(&mut buf[offset..], purpose)?.len();
        }
        Ok(&mut buf[offset..])
    }
}

// Vec<T>

impl<T: RawEncode> RawFixedBytes for Vec<T> {
    fn raw_min_bytes() -> Option<usize> {
        Some(1)
    }
}

impl<T: RawEncode> RawEncode for Vec<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        self.as_slice().raw_measure(purpose)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.as_slice().raw_encode(buf, purpose)
    }
}

impl<'de, T: RawEncode + RawDecode<'de>> RawDecode<'de> for Vec<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        if buf.len() < Self::raw_min_bytes().unwrap() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for vec",
            ));
        }
        let (ulen, buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        let mut offset: usize = 0;
        // println!("vec len {}", len);
        if len > u32::max_value() as usize {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "vec len overflow",
            ));
        }
        let mut vec = std::vec::Vec::with_capacity(len);
        for _ in 0..len {
            let (e, next) = T::raw_decode(&buf[offset..])?;
            offset = buf.len() - next.len();
            vec.push(e);
        }
        Ok((vec, &buf[offset..]))
    }
}

// HashSet<T>

impl<T: RawEncode> RawEncode for HashSet<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.len());
        let mut bytes = ulen.raw_measure(purpose).unwrap();
        for e in self {
            bytes += e.raw_measure(purpose)?;
        }
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let ulen = USize(self.len());
        let mut buf = ulen.raw_encode(buf, purpose)?;
        for e in self {
            buf = e.raw_encode(buf, purpose)?;
        }
        Ok(buf)
    }
}

impl<'de, T: Eq + Hash + RawEncode + RawDecode<'de>> RawDecode<'de> for HashSet<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ulen, mut buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        let mut set = HashSet::with_capacity(len as usize);
        for _ in 0..len {
            let (e, _buf) = T::raw_decode(buf)?;
            buf = _buf;
            set.insert(e);
        }

        Ok((set, buf))
    }
}

// HashMap<K,V>

impl<K: RawEncode + std::cmp::Ord + std::hash::Hash, V: RawEncode> RawEncode for HashMap<K, V> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.len());
        let mut size = ulen.raw_measure(purpose).unwrap();
        for (key, value) in self {
            size += key.raw_measure(purpose)? + value.raw_measure(purpose)?;
        }
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let ulen = USize(self.len());
        let mut buf = ulen.raw_encode(buf, purpose)?;

        // stable sort
        let mut keys: Vec<&K> = self.keys().collect();
        keys.sort();
        for key in keys {
            buf = key.raw_encode(buf, purpose)?;

            let value = self.get(key).unwrap();
            buf = value.raw_encode(buf, purpose)?;
        }
        // for (key, value) in self {
        //     buf = key.raw_encode(buf, purpose)?;
        //     buf = value.raw_encode(buf, purpose)?;
        // }
        Ok(buf)
    }
}

impl<'de, K: RawDecode<'de> + Hash + Eq, V: RawDecode<'de>> RawDecode<'de> for HashMap<K, V> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ulen, mut buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        let mut map = HashMap::new();
        for _ in 0..len {
            let (key, tmp_buf) = K::raw_decode(buf)?;
            let (v, tmp_buf) = V::raw_decode(tmp_buf)?;
            buf = tmp_buf;
            map.insert(key, v);
        }
        Ok((map, buf))
    }
}

// BTreeSet<T>

impl<T: RawEncode> RawEncode for BTreeSet<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.len());
        let mut bytes = ulen.raw_measure(purpose).unwrap();
        for e in self {
            bytes += e.raw_measure(purpose)?;
        }
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let ulen = USize(self.len());
        let mut buf = ulen.raw_encode(buf, purpose)?;
        for e in self {
            buf = e.raw_encode(buf, purpose)?;
        }
        Ok(buf)
    }
}

impl<'de, T: Ord + RawDecode<'de>> RawDecode<'de> for BTreeSet<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ulen, mut buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        let mut set = BTreeSet::new();
        for _ in 0..len {
            let (e, _buf) = T::raw_decode(buf)?;
            buf = _buf;
            set.insert(e);
        }

        Ok((set, buf))
    }
}

// BTreeMap<K,V>
use std::collections::BTreeMap;

impl<K: RawEncode + std::cmp::Ord, V: RawEncode> RawEncode for BTreeMap<K, V> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.len());
        let mut size = ulen.raw_measure(purpose).unwrap();
        for (key, value) in self {
            size += key.raw_measure(purpose)? + value.raw_measure(purpose)?;
        }
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let ulen = USize(self.len());
        let mut buf = ulen.raw_encode(buf, purpose)?;

        for (key, value) in self {
            buf = key.raw_encode(buf, purpose)?;
            buf = value.raw_encode(buf, purpose)?;
        }

        Ok(buf)
    }
}

impl<'de, K: RawDecode<'de> + std::cmp::Ord + Eq, V: RawDecode<'de>> RawDecode<'de>
    for BTreeMap<K, V>
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ulen, mut buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        let mut map = BTreeMap::new();
        for _ in 0..len {
            let (key, tmp_buf) = K::raw_decode(buf)?;
            let (v, tmp_buf) = V::raw_decode(tmp_buf)?;
            buf = tmp_buf;
            map.insert(key, v);
        }
        Ok((map, buf))
    }
}

// Mutex

use std::sync::Mutex;

impl<V: RawEncode> RawEncode for Mutex<V> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let v = self.lock().unwrap();
        v.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let v = self.lock().unwrap();
        v.raw_encode(buf, purpose)
    }
}

impl<'de, V: RawDecode<'de>> RawDecode<'de> for Mutex<V> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (v, buf) = V::raw_decode(buf)?;
        Ok((Mutex::new(v), buf))
    }
}

// Arc
use std::sync::Arc;

impl<V: RawEncode> RawEncode for Arc<V> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let v = self.deref();
        v.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let v = self.deref();
        v.raw_encode(buf, purpose)
    }
}

impl<'de, V: RawDecode<'de>> RawDecode<'de> for Arc<V> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (v, buf) = V::raw_decode(buf)?;
        Ok((Arc::new(v), buf))
    }
}

// Atomici32

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

impl RawEncode for AtomicI32 {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let v = self.load(Ordering::SeqCst);
        v.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let v = self.load(Ordering::SeqCst);
        v.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for AtomicI32 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (v, buf) = i32::raw_decode(buf)?;
        Ok((AtomicI32::new(v), buf))
    }
}

// Atomicu32

impl RawEncode for AtomicU32 {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let v = self.load(Ordering::SeqCst);
        v.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let v = self.load(Ordering::SeqCst);
        v.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for AtomicU32 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (v, buf) = u32::raw_decode(buf)?;
        Ok((AtomicU32::new(v), buf))
    }
}

// tuple

impl<T: RawEncode, U: RawEncode> RawEncode for (T, U) {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = self.0.raw_measure(purpose)? + self.1.raw_measure(purpose)?;
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.0.raw_encode(buf, purpose)?;
        let buf = self.1.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de, T: RawEncode + RawDecode<'de>, U: RawEncode + RawDecode<'de>> RawDecode<'de> for (T, U) {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (e, buf) = T::raw_decode(buf)?;
        let (u, buf) = U::raw_decode(buf)?;
        Ok(((e, u), buf))
    }
}

impl<T1: RawEncode, T2: RawEncode, T3: RawEncode> RawEncode for (T1, T2, T3) {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = self.0.raw_measure(purpose)?
            + self.1.raw_measure(purpose)?
            + self.2.raw_measure(purpose)?;
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.0.raw_encode(buf, purpose)?;
        let buf = self.1.raw_encode(buf, purpose)?;
        let buf = self.2.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<
        'de,
        T1: RawEncode + RawDecode<'de>,
        T2: RawEncode + RawDecode<'de>,
        T3: RawEncode + RawDecode<'de>,
    > RawDecode<'de> for (T1, T2, T3)
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (t1, buf) = T1::raw_decode(buf)?;
        let (t2, buf) = T2::raw_decode(buf)?;
        let (t3, buf) = T3::raw_decode(buf)?;
        Ok(((t1, t2, t3), buf))
    }
}

impl<T1: RawEncode, T2: RawEncode, T3: RawEncode, T4: RawEncode> RawEncode for (T1, T2, T3, T4) {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = self.0.raw_measure(purpose)?
            + self.1.raw_measure(purpose)?
            + self.2.raw_measure(purpose)?
            + self.3.raw_measure(purpose)?;
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.0.raw_encode(buf, purpose)?;
        let buf = self.1.raw_encode(buf, purpose)?;
        let buf = self.2.raw_encode(buf, purpose)?;
        let buf = self.3.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<
        'de,
        T1: RawEncode + RawDecode<'de>,
        T2: RawEncode + RawDecode<'de>,
        T3: RawEncode + RawDecode<'de>,
        T4: RawEncode + RawDecode<'de>,
    > RawDecode<'de> for (T1, T2, T3, T4)
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (t1, buf) = T1::raw_decode(buf)?;
        let (t2, buf) = T2::raw_decode(buf)?;
        let (t3, buf) = T3::raw_decode(buf)?;
        let (t4, buf) = T4::raw_decode(buf)?;
        Ok(((t1, t2, t3, t4), buf))
    }
}

impl<T1: RawEncode, T2: RawEncode, T3: RawEncode, T4: RawEncode, T5: RawEncode> RawEncode
    for (T1, T2, T3, T4, T5)
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = self.0.raw_measure(purpose)?
            + self.1.raw_measure(purpose)?
            + self.2.raw_measure(purpose)?
            + self.3.raw_measure(purpose)?
            + self.4.raw_measure(purpose)?;
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.0.raw_encode(buf, purpose)?;
        let buf = self.1.raw_encode(buf, purpose)?;
        let buf = self.2.raw_encode(buf, purpose)?;
        let buf = self.3.raw_encode(buf, purpose)?;
        let buf = self.4.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<
        'de,
        T1: RawEncode + RawDecode<'de>,
        T2: RawEncode + RawDecode<'de>,
        T3: RawEncode + RawDecode<'de>,
        T4: RawEncode + RawDecode<'de>,
        T5: RawEncode + RawDecode<'de>,
    > RawDecode<'de> for (T1, T2, T3, T4, T5)
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (t1, buf) = T1::raw_decode(buf)?;
        let (t2, buf) = T2::raw_decode(buf)?;
        let (t3, buf) = T3::raw_decode(buf)?;
        let (t4, buf) = T4::raw_decode(buf)?;
        let (t5, buf) = T5::raw_decode(buf)?;
        Ok(((t1, t2, t3, t4, t5), buf))
    }
}

// &str只是单纯用来计算编码后的大小
impl RawFixedBytes for &str {
    fn raw_min_bytes() -> Option<usize> {
        u16::raw_bytes()
    }
}

impl RawEncode for &str {
    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unreachable!();
    }

    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = u16::raw_bytes().unwrap() + self.as_bytes().len();
        Ok(bytes)
    }
}

impl RawFixedBytes for String {
    fn raw_min_bytes() -> Option<usize> {
        u16::raw_bytes()
    }
}

impl RawEncode for String {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = u16::raw_bytes().unwrap() + self.as_bytes().len();
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let total_bytes = self.raw_measure(purpose)?;
        if buf.len() < total_bytes {
            let msg = format!(
                "not enough buffer for String: bytes={}, buf={}",
                total_bytes,
                buf.len()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        // 检查是否超出最大长度限制
        let bytes = self.as_bytes().len();
        if bytes > u16::MAX.into() {
            let msg = format!(
                "String extend length max limit: bytes={}, limit={}",
                bytes,
                u16::MAX
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let buf = (bytes as u16).raw_encode(buf, purpose)?;
        if bytes == 0 {
            Ok(buf)
        } else {
            unsafe {
                std::ptr::copy::<u8>(self.as_bytes().as_ptr() as *mut u8, buf.as_mut_ptr(), bytes);
            }
            Ok(&mut buf[bytes..])
        }
    }
}

impl<'de> RawDecode<'de> for String {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for String: bytes={}, buf={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let (len, buf) = u16::raw_decode(buf)?;
        if len == 0 {
            Ok((String::from(""), buf))
        } else {
            let mut bytes_buf = Vec::<u8>::with_capacity(len as usize);
            unsafe {
                std::ptr::copy::<u8>(buf.as_ptr(), bytes_buf.as_mut_ptr(), len as usize);
                bytes_buf.set_len(len as usize);
            }

            let str = String::from_utf8(bytes_buf).or_else(|_| {
                Err(BuckyError::new(
                    BuckyErrorCode::CryptoError,
                    "ParseUtf8Error",
                ))
            })?;
            Ok((str, &buf[len as usize..]))
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VarString(pub String);

impl VarString {
    pub fn value(&self) -> &String {
        &self.0
    }
}

impl Default for VarString {
    fn default() -> Self {
        Self(String::default())
    }
}

impl ToString for VarString {
    #[inline]
    fn to_string(&self) -> String {
        self.0.to_owned()
    }
}

impl AsRef<str> for VarString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl AsMut<str> for VarString {
    #[inline]
    fn as_mut(&mut self) -> &mut str {
        self.0.as_mut()
    }
}

impl Deref for VarString {
    type Target = String;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for VarString {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl RawFixedBytes for VarString {
    fn raw_min_bytes() -> Option<usize> {
        Some(1)
    }
}

impl RawEncode for VarString {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let ulen = USize(self.0.len());

        let bytes = ulen.raw_measure(purpose).unwrap() + self.0.as_bytes().len();
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let total_bytes = self.raw_measure(purpose)?;
        if buf.len() < total_bytes {
            let msg = format!(
                "not enough buffer for String: bytes={}, buf={}",
                total_bytes,
                buf.len()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let bytes = self.0.as_bytes().len();
        let ulen = USize(bytes);
        let buf = ulen.raw_encode(buf, purpose)?;
        if bytes == 0 {
            Ok(buf)
        } else {
            unsafe {
                std::ptr::copy::<u8>(
                    self.0.as_bytes().as_ptr() as *mut u8,
                    buf.as_mut_ptr(),
                    bytes,
                );
            }
            Ok(&mut buf[bytes..])
        }
    }
}

impl<'de> RawDecode<'de> for VarString {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for String: bytes={}, buf={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let (ulen, buf) = USize::raw_decode(buf)?;
        let len = ulen.value();
        if len == 0 {
            Ok((VarString(String::from("")), buf))
        } else {
            let mut bytes_buf = Vec::<u8>::with_capacity(len);
            unsafe {
                std::ptr::copy::<u8>(buf.as_ptr(), bytes_buf.as_mut_ptr(), len as usize);
                bytes_buf.set_len(len as usize);
            }

            let str = String::from_utf8(bytes_buf).or_else(|_| {
                Err(BuckyError::new(
                    BuckyErrorCode::CryptoError,
                    "ParseUtf8Error",
                ))
            })?;
            Ok((VarString(str), &buf[len as usize..]))
        }
    }
}

impl<T: RawEncode, U: ArrayLength<T>> RawFixedBytes for GenericArray<T, U> {
    fn raw_min_bytes() -> Option<usize> {
        Some(U::to_usize())
    }
}

impl<T: RawEncode, U: ArrayLength<T>> RawEncode for GenericArray<T, U> {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = U::to_usize();
        Ok(bytes)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = self.raw_measure(purpose)?;
        if buf.len() < bytes {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for GenericArray",
            ));
        }
        unsafe {
            std::ptr::copy::<u8>(self.as_slice().as_ptr() as *mut u8, buf.as_mut_ptr(), bytes);
        }
        Ok(&mut buf[bytes..])
    }
}

impl<'de, T: RawEncode + RawDecode<'de> + Default, U: ArrayLength<T>> RawDecode<'de>
    for GenericArray<T, U>
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_min_bytes().unwrap();
        if buf.len() < bytes {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for GenericArray",
            ));
        }
        let mut obj = GenericArray::<T, U>::default();
        unsafe {
            std::ptr::copy::<u8>(
                buf.as_ptr(),
                obj.as_mut_slice().as_mut_ptr() as *mut u8,
                bytes,
            );
        }
        Ok((obj, &buf[bytes..]))
    }
}

// SizedOwnedData
// 包含编码大小的数据段，从buf中拷贝出来
use primitive_types::H256;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

#[derive(Clone, std::cmp::PartialEq)]
pub struct SizedOwnedData<T: From<usize>>(std::vec::Vec<u8>, Option<PhantomData<T>>);

impl<T: From<usize>> SizedOwnedData<T> {
    pub fn take(self) -> std::vec::Vec<u8> {
        self.0
    }

    pub fn clear(&mut self) -> std::vec::Vec<u8> {
        let empty = vec![0; 0];
        std::mem::replace(&mut self.0, empty)
    }
}

impl<T> std::fmt::Debug for SizedOwnedData<T>
where
    T: From<usize>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SizedOwnedData: {}", hex::encode(self.0.as_slice()),)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SizeU8(pub u8);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SizeU16(pub u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SizeU32(pub u32);

impl From<usize> for SizeU8 {
    fn from(v: usize) -> Self {
        SizeU8(v as u8)
    }
}

impl From<SizeU8> for usize {
    fn from(v: SizeU8) -> Self {
        v.0 as usize
    }
}

impl RawFixedBytes for SizeU8 {
    fn raw_bytes() -> Option<usize> {
        u8::raw_bytes()
    }
}

impl RawEncode for SizeU8 {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for SizeU8 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        u8::raw_decode(buf).map(|(o, buf)| (SizeU8(o), buf))
    }
}

impl From<usize> for SizeU16 {
    fn from(v: usize) -> Self {
        SizeU16(v as u16)
    }
}

impl From<SizeU16> for usize {
    fn from(v: SizeU16) -> Self {
        v.0 as usize
    }
}

impl RawFixedBytes for SizeU16 {
    fn raw_bytes() -> Option<usize> {
        u16::raw_bytes()
    }
}

impl RawEncode for SizeU16 {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for SizeU16 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        u16::raw_decode(buf).map(|(o, buf)| (SizeU16(o), buf))
    }
}

impl From<usize> for SizeU32 {
    fn from(v: usize) -> Self {
        SizeU32(v as u32)
    }
}

impl From<SizeU32> for usize {
    fn from(v: SizeU32) -> Self {
        v.0 as usize
    }
}

impl RawFixedBytes for SizeU32 {
    fn raw_bytes() -> Option<usize> {
        u32::raw_bytes()
    }
}

impl RawEncode for SizeU32 {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for SizeU32 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        u32::raw_decode(buf).map(|(o, buf)| (SizeU32(o), buf))
    }
}

impl<T: From<usize>> SizedOwnedData<T> {
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<T: From<usize>> AsRef<std::vec::Vec<u8>> for SizedOwnedData<T> {
    fn as_ref(&self) -> &std::vec::Vec<u8> {
        &self.0
    }
}

impl<T: From<usize>> AsMut<std::vec::Vec<u8>> for SizedOwnedData<T> {
    fn as_mut(&mut self) -> &mut std::vec::Vec<u8> {
        &mut self.0
    }
}

impl<T: From<usize>> From<SizedOwnedData<T>> for std::vec::Vec<u8> {
    fn from(s: SizedOwnedData<T>) -> std::vec::Vec<u8> {
        s.0
    }
}

impl<T: From<usize>> From<&std::vec::Vec<u8>> for SizedOwnedData<T> {
    fn from(v: &std::vec::Vec<u8>) -> Self {
        Self((*v).clone(), None)
    }
}

impl<T: From<usize>> From<std::vec::Vec<u8>> for SizedOwnedData<T> {
    fn from(v: std::vec::Vec<u8>) -> Self {
        Self(v, None)
    }
}

impl<T: From<usize> + RawFixedBytes> RawFixedBytes for SizedOwnedData<T> {
    fn raw_min_bytes() -> Option<usize> {
        T::raw_bytes()
    }
}

impl<T: From<usize> + RawFixedBytes + RawEncode> RawEncode for SizedOwnedData<T> {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(T::raw_bytes().unwrap() + self.0.len())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let t: T = self.0.len().into();
        let buf = t.raw_encode(buf, purpose)?;
        if buf.len() < self.0.len() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for SizedOwnedData",
            ));
        }
        unsafe {
            std::ptr::copy(self.0.as_ptr(), buf.as_mut_ptr(), self.0.len());
        }
        let buf = &mut buf[self.0.len()..];
        Ok(buf)
    }
}

impl<'de, T: From<usize> + RawDecode<'de> + Into<usize>> RawDecode<'de> for SizedOwnedData<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (len, buf) = T::raw_decode(buf)?;
        let size: usize = len.into();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] buf not enough for SizedOwnedData",
            ));
        }

        let mut data = Vec::with_capacity(size);
        unsafe {
            data.set_len(size);
            std::ptr::copy(buf.as_ptr(), data.as_mut_ptr(), size);
        }
        let buf = &buf[data.len()..];
        Ok((Self(data, None), buf))
    }
}

// SizedSharedData
// 包含编码大小的数据段，从buf引用
pub struct SizedSharedData<'de>(&'de [u8]);
impl<'de> RawDecode<'de> for SizedSharedData<'de> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (len, buf) = u16::raw_decode(buf)?;
        Ok((Self(&buf[len as usize..]), buf))
    }
}

// 拷贝的结尾数据
#[derive(Clone, Debug)]
pub struct TailedOwnedData(std::vec::Vec<u8>);

impl std::fmt::Display for TailedOwnedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tail data len {}", self.0.len())
    }
}

impl From<std::vec::Vec<u8>> for TailedOwnedData {
    fn from(v: std::vec::Vec<u8>) -> Self {
        Self(v)
    }
}

impl From<&[u8]> for TailedOwnedData {
    fn from(s: &[u8]) -> Self {
        let v = Vec::from(s);
        Self(v)
    }
}
//
// impl AsRef<std::vec::Vec<u8>> for TailedOwnedData {
//     fn as_ref(&self) -> &std::vec::Vec<u8> {
//         &self.0
//     }
// }

impl AsRef<[u8]> for TailedOwnedData {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl RawEncode for TailedOwnedData {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u32::raw_bytes().unwrap() + self.0.len())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        //let buf = u32::from(self.0.len() as u32).raw_encode(buf, purpose)?;
        if buf.len() < self.0.len() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for TailedOwnedData",
            ));
        }
        unsafe {
            std::ptr::copy(self.0.as_ptr(), buf.as_mut_ptr(), self.0.len());
        }
        let buf = &mut buf[self.0.len()..];
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for TailedOwnedData {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        //let (len, buf)= u32::raw_decode(buf)?;
        let size = buf.len();
        let mut data = std::vec::Vec::with_capacity(size);
        unsafe {
            data.set_len(size);
            std::ptr::copy(buf.as_ptr(), data.as_mut_ptr(), size);
        }
        let buf = &buf[data.len()..];
        Ok((Self(data), buf))
    }
}

// TailedSharedData
// 共享的结尾数据
pub struct TailedSharedData<'de>(&'de [u8]);

impl<'a> From<&'a [u8]> for TailedSharedData<'a> {
    fn from(v: &'a [u8]) -> Self {
        TailedSharedData(v)
    }
}

impl AsRef<[u8]> for TailedSharedData<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl RawEncode for TailedSharedData<'_> {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(self.0.len())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if buf.len() < self.0.len() {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for TailedSharedData",
            ));
        }
        unsafe {
            std::ptr::copy(self.0.as_ptr(), buf.as_mut_ptr(), self.0.len());
        }
        let buf = &mut buf[self.0.len()..];
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for TailedSharedData<'de> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        Ok((Self(buf), &buf[buf.len()..]))
    }
}

impl RawEncode for IpAddr {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(match self {
            IpAddr::V4(_) => u8::raw_bytes().unwrap() + 4,
            IpAddr::V6(_) => u8::raw_bytes().unwrap() + 16,
        })
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            IpAddr::V4(ref sock_addr) => {
                let buf = (0u8).raw_encode(buf, purpose)?;

                if buf.len() < 4 {
                    Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for IpAddr",
                    ))
                } else {
                    unsafe {
                        std::ptr::copy(
                            sock_addr.octets().as_ptr() as *const u8,
                            buf.as_mut_ptr(),
                            4,
                        );
                    }
                    Ok(&mut buf[4..])
                }
            }
            IpAddr::V6(ref sock_addr) => {
                let buf = (1u8).raw_encode(buf, purpose)?;

                if buf.len() < 16 {
                    Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for IpAddr",
                    ))
                } else {
                    unsafe {
                        std::ptr::copy(
                            sock_addr.octets().as_ptr() as *const u8,
                            buf.as_mut_ptr(),
                            16,
                        );
                    }
                    Ok(&mut buf[16..])
                }
            }
        }
    }
}

impl<'de> RawDecode<'de> for IpAddr {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ty, buf) = u8::raw_decode(buf)?;
        match ty {
            0 => {
                if buf.len() < 4 {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for IpAddr",
                    ));
                }
                let addr = IpAddr::V4(Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]));
                Ok((addr, &buf[4..]))
            }
            1 => {
                if buf.len() < 16 {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for IpAddr",
                    ));
                }
                let s = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u16, 8) };
                // TOFIX: flow and scope
                let addr = IpAddr::V6(Ipv6Addr::new(
                    s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
                ));
                Ok((addr, &buf[16..]))
            }
            _ => Err(BuckyError::new(BuckyErrorCode::NotSupport, "NotSupport")),
        }
    }
}

impl<T: RawEncode> RawEncode for Option<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size = if let Some(t) = self {
            t.raw_measure(purpose)? + 1
        } else {
            1
        };
        Ok(size)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if self.is_some() {
            let buf = 1u8.raw_encode(buf, purpose)?;
            self.as_ref().unwrap().raw_encode(buf, purpose)
        } else {
            0u8.raw_encode(buf, purpose)
        }
    }
}

pub struct OptionRef<'o, T>(Option<&'o T>);

impl<'o, T> OptionRef<'o, T> {
    pub fn option(&self) -> Option<&'o T> {
        self.0
    }
}

impl<'o, T> From<Option<&'o T>> for OptionRef<'o, T> {
    fn from(opt: Option<&'o T>) -> Self {
        Self(opt)
    }
}

// impl<'o,T> From<OptionRef<'o,T>> for Option<&'o T> {
//     fn from(opt: OptionRef<'o,T>) -> Self {
//         opt.0
//     }
// }

// impl<'o,T> From<OptionRef<'_,T>> for Option<&T> {
//     fn from(opt: OptionRef<'_,T>) -> Self {
//         opt.0
//     }
// }

impl<'o, T: RawEncode> RawEncode for OptionRef<'o, T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size = 1 + match self.0 {
            Some(t) => t.raw_measure(purpose)?,
            None => 0,
        };
        Ok(size)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if self.0.is_some() {
            let buf = 1u8.raw_encode(buf, purpose)?;
            self.0.as_ref().unwrap().raw_encode(buf, purpose)
        } else {
            0u8.raw_encode(buf, purpose)
        }
    }
}

impl<'de, T: RawDecode<'de>> RawDecode<'de> for Option<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (option, buf) = u8::raw_decode(buf)?;

        let (t, buf) = if option == 1 {
            let (t, buf) = T::raw_decode(buf)?;
            (Some(t), buf)
        } else {
            (None, buf)
        };

        Ok((t, buf))
    }
}

impl<T: RawEncode, E: RawEncode> RawEncode for Result<T, E> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            Ok(t) => Ok(u8::raw_bytes().unwrap() + t.raw_measure(purpose)?),
            Err(e) => Ok(u8::raw_bytes().unwrap() + e.raw_measure(purpose)?),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Ok(t) => {
                let buf = 0u8.raw_encode(buf, purpose)?;
                let buf = t.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            Err(e) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                let buf = e.raw_encode(buf, purpose)?;
                Ok(buf)
            }
        }
    }
}

impl<'de, T: RawDecode<'de>, E: RawDecode<'de>> RawDecode<'de> for Result<T, E> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (ty, buf) = u8::raw_decode(buf)?;
        match ty {
            0 => {
                let (t, buf) = T::raw_decode(buf)?;
                Ok((Ok(t), buf))
            }
            1 => {
                let (e, buf) = E::raw_decode(buf)?;
                Ok((Err(e), buf))
            }
            _ => Err(BuckyError::new(BuckyErrorCode::NotSupport, "NotSupport")),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TypeBuffer<T>
where
    T: RawEncode + for<'de> RawDecode<'de>,
{
    obj: T,
}

impl<T> From<T> for TypeBuffer<T>
where
    for<'de> T: RawEncode + RawDecode<'de>,
{
    fn from(obj: T) -> Self {
        Self { obj }
    }
}

impl<T> TypeBuffer<T>
where
    for<'de> T: RawEncode + RawDecode<'de>,
{
    pub fn into(self) -> T {
        self.obj
    }

    pub fn get_obj(&self) -> &T {
        &self.obj
    }

    pub fn get_mut_obj(&mut self) -> &mut T {
        &mut self.obj
    }
}

impl<T> Deref for TypeBuffer<T>
where
    for<'de> T: RawEncode + RawDecode<'de>,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.obj
    }
}

impl<T> DerefMut for TypeBuffer<T>
where
    for<'de> T: RawEncode + RawDecode<'de>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.obj
    }
}

impl<T> RawEncode for TypeBuffer<T>
where
    for<'de> T: RawEncode + RawDecode<'de>,
{
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let obj_len = self.obj.raw_measure(purpose)?;
        let bytes = USize(obj_len).raw_measure(purpose)? + self.obj.raw_measure(purpose)?;
        Ok(bytes)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = self.obj.raw_measure(purpose)?;
        let buf = USize(bytes).raw_encode(buf, purpose)?;
        let buf = self.obj.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de, T> RawDecode<'de> for TypeBuffer<T>
where
    for<'e> T: RawEncode + RawDecode<'e>,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (_, buf) = USize::raw_decode(buf)?;
        let (obj, buf) = T::raw_decode(buf)?;
        Ok((Self { obj }, buf))
    }
}

impl RawFixedBytes for H256 {
    fn raw_bytes() -> Option<usize> {
        Some(32)
    }
}

impl RawEncode for H256 {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(32)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = self.raw_measure(purpose)?;
        if buf.len() < bytes {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for H256",
            ));
        }
        unsafe {
            std::ptr::copy::<u8>(self.as_ptr(), buf.as_mut_ptr(), bytes);
        }
        Ok(&mut buf[bytes..])
    }
}

impl<'de> RawDecode<'de> for H256 {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for H256",
            ));
        }
        let mut obj = Self::default();
        unsafe {
            std::ptr::copy::<u8>(buf.as_ptr(), obj.as_mut_ptr(), bytes);
        }
        Ok((obj, &buf[bytes..]))
    }
}

// Range<T>
impl<T: RawEncode + RawFixedBytes> RawFixedBytes for Range<T> {
    fn raw_bytes() -> Option<usize> {
        if let Some(s) = T::raw_bytes() {
            Some(s + s)
        } else {
            None
        }
    }
}

impl<T: RawEncode> RawEncode for Range<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let bytes = self.start.raw_measure(purpose)? + self.end.raw_measure(purpose)?;
        Ok(bytes)
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.start.raw_encode(buf, purpose)?;
        self.end.raw_encode(buf, purpose)
    }
}

impl<'de, T: RawEncode + RawDecode<'de>> RawDecode<'de> for Range<T> {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (start, buf) = T::raw_decode(buf)?;
        let (end, buf) = T::raw_decode(buf)?;
        Ok((Range { start, end }, buf))
    }
}

#[cfg(test)]
mod raw_codec_test {
    use crate::*;

    fn test_var_string_codec(v: &str) {
        let vs = VarString(v.to_owned());
        let value = vs.to_vec().unwrap();

        let vs_2 = VarString::clone_from_slice(&value).unwrap();
        assert_eq!(vs_2.0, vs.0);
    }

    fn test_string_codec(v: &str) {
        let vs = v.to_owned();
        let value = vs.to_vec().unwrap();

        let vs_2 = String::clone_from_slice(&value).unwrap();
        assert_eq!(vs_2, vs);
    }

    fn test_codec<T>(v: &T)
    where
        for<'de> T: RawEncode + RawDecode<'de> + std::fmt::Debug + std::cmp::PartialEq,
    {
        let value = v.to_vec().unwrap();
        let (v2, remain) = T::raw_decode(&value).unwrap();
        assert!(remain.len() == 0);
        assert_eq!(*v, v2);
    }

    #[test]
    fn test_string() {
        let mut s = "See scope for the exact definition, and safety guidelines. The simplest and safest API is scope_and_block, used as follows".to_owned();
        test_var_string_codec(&s);
        test_string_codec(&s);

        while s.len() < (u8::MAX as usize) * 2 {
            s += &s.clone();
        }
        test_var_string_codec(&s);

        while s.len() < (u16::MAX as usize) * 2 {
            s += &s.clone();
        }
        test_var_string_codec(&s);

        // 超出了65536大小，应该会编码失败
        {
            s.to_vec().unwrap_err();
        }
    }

    fn test_size_codec(v: u64, except_len: usize) {
        assert!(v <= usize::MAX as u64);

        let v = USize(v as usize);
        let buf = v.to_vec().unwrap();
        assert_eq!(buf.len(), except_len);
        let v1 = USize::clone_from_slice(&buf).unwrap();
        assert_eq!(v1, v);
    }

    #[test]
    fn test_size() {
        const U6_MAX: u64 = (u8::MAX >> 2) as u64;
        const U14_MAX: u64 = (u16::MAX >> 2) as u64;
        const U22_MAX: u64 = (u32::MAX >> 10) as u64;
        const U30_MAX: u64 = (u32::MAX >> 2) as u64;
        const VARSIZE_SUB_2_MAX: u64 = (u64::MAX >> 2) as u64;
        println!(
            "{}, {}, {}, {}",
            U6_MAX, U14_MAX, U30_MAX, VARSIZE_SUB_2_MAX
        );

        test_size_codec(U6_MAX, 1);
        test_size_codec(U6_MAX + 1, 2);
        test_size_codec(U14_MAX, 2);
        test_size_codec(U14_MAX + 1, 4);
        test_size_codec(U30_MAX, 4);
        test_size_codec(U30_MAX + 1, 8);
        test_size_codec(U30_MAX + 100, 8);
        test_size_codec(VARSIZE_SUB_2_MAX, 8);

        {
            let v = USize(U6_MAX as usize);
            let buf = v.to_vec().unwrap();
            assert_eq!(buf.len(), 1);
            let v1 = USize::clone_from_slice(&buf).unwrap();
            assert_eq!(v1, v);
        }
        {
            let v = USize(64);
            let buf = v.to_vec().unwrap();
            assert_eq!(buf.len(), 2);
            let v1 = USize::clone_from_slice(&buf).unwrap();
            assert_eq!(v1, v);
        }

        {
            let v = USize(16383);
            let buf = v.to_vec().unwrap();
            assert_eq!(buf.len(), 2);
            let v1 = USize::clone_from_slice(&buf).unwrap();
            assert_eq!(v1, v);
        }
        {
            let v = USize(16384);
            let buf = v.to_vec().unwrap();
            assert_eq!(buf.len(), 4);
            let v1 = USize::clone_from_slice(&buf).unwrap();
            assert_eq!(v1, v);
        }

        {
            let size = SizeU8(100);
            test_codec(&size);

            let size = SizeU8(u8::MAX);
            test_codec(&size);
        }

        {
            let size = SizeU16(100);
            test_codec(&size);

            let size = SizeU16(u16::MAX);
            test_codec(&size);
        }

        {
            let size = SizeU32(100);
            test_codec(&size);

            let size = SizeU32(u16::MAX.into());
            test_codec(&size);

            let size = SizeU32(u32::MAX.into());
            test_codec(&size);
        }
    }

    #[test]
    fn test_hash_set() {
        use std::collections::hash_set::HashSet;

        let s = "See scope for the exact definition, and safety guidelines. The simplest and safest API is scope_and_block, used as follows".to_owned();
        let s2 = (s.clone() + &s.clone()).to_owned();
        let s3 = "xxxxx".to_owned();
        let mut set = HashSet::new();
        set.insert(s);
        set.insert(s2);
        set.insert(s3);

        test_codec(&set);

        let mut set = HashSet::new();
        set.insert(0);
        set.insert(100);
        set.insert(1024);
        set.insert(88);

        test_codec(&set);
    }

    #[test]
    fn test_hash_map() {
        use std::collections::HashMap;

        let s = "See scope for the exact definition, and safety guidelines. The simplest and safest API is scope_and_block, used as follows".to_owned();
        let s2 = (s.clone() + &s.clone()).to_owned();
        let s3 = "xxxxx".to_owned();
        let mut set = HashMap::new();
        set.insert(s.clone(), 100);
        set.insert(s2.clone(), 99);
        set.insert(s3.clone(), 88);

        test_codec(&set);

        let mut set = HashMap::new();
        set.insert(0, s);
        set.insert(100, s2);
        set.insert(1024, s3.clone());
        set.insert(88, s3);

        test_codec(&set);
    }
}
