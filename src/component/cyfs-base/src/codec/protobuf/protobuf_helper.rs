use crate::*;

use std::{convert::TryFrom, str::FromStr};
use std::collections::HashMap;

// 为了读取CodedOutputStream里面的position字段
// TODO 确保protobuf的版本，如果此结构体发生变化，需要同步，否则会导致读取错误
mod stream_pos_retreve_helper {
    enum OutputTarget<'a> {
        Write(&'a mut dyn std::io::Write, Vec<u8>),
        Vec(&'a mut Vec<u8>),
        Bytes,
    }
    /// Buffered write with handy utilities
    pub struct CodedOutputStream<'a> {
        target: OutputTarget<'a>,
        // alias to buf from target
        buffer: &'a mut [u8],
        // within buffer
        pub position: usize,
    }
}

pub struct ProtobufMessageCodecHelper {}

impl ProtobufMessageCodecHelper {
    pub fn raw_measure(
        value: impl ::protobuf::Message,
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        let size = value.compute_size() as usize;

        Ok(size)
    }

    pub fn raw_encode<'a>(
        value: impl ::protobuf::Message,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = value.compute_size() as usize;
        let mut stream = ::protobuf::CodedOutputStream::bytes(buf);
        value.write_to(&mut stream).map_err(|e| {
            let msg = format!("encode protobuf::Message to stream error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::OutOfLimit, msg)
        })?;

        //let stream_exposed: stream_pos_retreve_helper::CodedOutputStream =
        //    unsafe { std::mem::transmute(stream) };

        Ok(&mut buf[size..])
    }

    // 需要使用精确长度的buf来decode
    pub fn raw_decode<'de, T>(buf: &'de [u8]) -> BuckyResult<(T, &'de [u8])>
    where
        T: ::protobuf::Message,
    {
        // buffer的size就是整个body_content的长度
        let size = buf.len();

        // 必须截取精确大小的buffer
        let mut stream = ::protobuf::CodedInputStream::from_bytes(buf);
        let value = T::parse_from(&mut stream).map_err(|e| {
            let msg = format!("decode protobuf::Message from stream error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        assert_eq!(stream.pos() as usize, size);

        Ok((value, &buf[size..]))
    }
}

pub struct ProtobufCodecHelper {}

impl ProtobufCodecHelper {
    pub fn raw_measure<'a, T, P>(
        value: &'a T,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize>
    where
        P: TryFrom<&'a T>,
        P: ::protobuf::Message,
        <P as TryFrom<&'a T>>::Error: std::fmt::Display,
    {
        let value: P = P::try_from(value).map_err(|e: <P as TryFrom<&'a T>>::Error| {
            let msg = format!("convert protobuf origin to protobuf::Message error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        ProtobufMessageCodecHelper::raw_measure(value, purpose)
    }

    pub fn raw_encode<'a, 'b, T, P>(
        value: &'b T,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]>
    where
        P: TryFrom<&'b T>,
        P: ::protobuf::Message,
        <P as TryFrom<&'b T>>::Error: std::fmt::Display,
    {
        let value: P = P::try_from(value).map_err(|e: <P as TryFrom<&'b T>>::Error| {
            let msg = format!("convert protobuf origin to protobuf::Message error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        ProtobufMessageCodecHelper::raw_encode(value, buf, purpose)
    }

    pub fn raw_decode<'de, T, P>(buf: &'de [u8]) -> BuckyResult<(T, &'de [u8])>
    where
        T: TryFrom<P>,
        P: ::protobuf::Message,
        <T as TryFrom<P>>::Error: std::fmt::Display,
    {
        let (msg, buf) = ProtobufMessageCodecHelper::raw_decode::<P>(buf)?;
        let value: T = T::try_from(msg).map_err(|e: <T as TryFrom<P>>::Error| {
            let msg = format!("convert protobuf message to type error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok((value, buf))
    }

    pub fn decode_buf<T>(buf: Vec<u8>) -> BuckyResult<T>
    where
        T: for<'de> RawDecode<'de>,
    {
        let (item, _) = T::raw_decode(&buf)?;

        Ok(item)
    }

    pub fn decode_string_list<T>(list: Vec<String>) -> BuckyResult<Vec<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
        BuckyError: From<<T as FromStr>::Err>,
    {
        let mut result = Vec::with_capacity(list.len());
        for s in list {
            let item = T::from_str(&s)?;

            result.push(item);
        }

        Ok(result)
    }

    pub fn encode_string_list<T>(list: &[T]) -> BuckyResult<::protobuf::RepeatedField<String>>
    where
        T: ToString,
    {
        let mut result = Vec::with_capacity(list.len());
        for item in list.iter() {
            let buf = item.to_string();

            result.push(buf);
        }

        Ok(result.into())
    }

    pub fn decode_buf_list<T>(list: impl Into<Vec<Vec<u8>>>) -> BuckyResult<Vec<T>>
    where
        T: for<'de> RawDecode<'de>,
    {
        let list: Vec<Vec<u8>> = list.into();
        let mut result = Vec::with_capacity(list.len());
        for buf in list {
            let (item, _) = T::raw_decode(&buf)?;

            result.push(item);
        }

        Ok(result)
    }

    pub fn encode_buf_list<T>(list: &[T]) -> BuckyResult<::protobuf::RepeatedField<Vec<u8>>>
    where
        T: RawEncode,
    {
        let mut result = Vec::with_capacity(list.len());
        for item in list.iter() {
            let buf = item.to_vec()?;

            result.push(buf);
        }

        Ok(result.into())
    }

    // 解码嵌套Message结构
    pub fn decode_nested_item<T, P>(item: P) -> BuckyResult<T>
    where
        T: TryFrom<P>,
        <T as TryFrom<P>>::Error: std::fmt::Display,
        BuckyError: From<<T as TryFrom<P>>::Error>,
    {
        let ret = T::try_from(item)?;

        Ok(ret)
    }

    // 编码到嵌套的Message结构
    pub fn encode_nested_item<'a, T, P>(item: &'a T) -> BuckyResult<P>
    where
        P: TryFrom<&'a T>,
        <P as TryFrom<&'a T>>::Error: std::fmt::Display,
        BuckyError: From<<P as TryFrom<&'a T>>::Error>,
    {
        let ret = P::try_from(item)?;

        Ok(ret)
    }

    // 解码支持嵌套TryFrom的结构体数组
    pub fn decode_nested_list<T, P>(list: impl Into<Vec<P>>) -> BuckyResult<Vec<T>>
    where
        T: TryFrom<P>,
        <T as TryFrom<P>>::Error: std::fmt::Display,
        BuckyError: From<<T as TryFrom<P>>::Error>,
    {
        let list: Vec<P> = list.into();
        let mut result = Vec::with_capacity(list.len());
        for v in list {
            let item = T::try_from(v)?;

            result.push(item);
        }

        Ok(result)
    }

    pub fn encode_nested_list<'a, T, P>(
        list: &'a Vec<T>,
    ) -> BuckyResult<::protobuf::RepeatedField<P>>
    where
        T: 'a,
        P: TryFrom<&'a T>,
        <P as TryFrom<&'a T>>::Error: std::fmt::Display,
        BuckyError: From<<P as TryFrom<&'a T>>::Error>,
    {
        let mut result = Vec::with_capacity(list.len());
        for v in list {
            let item = P::try_from(v)?;

            result.push(item);
        }

        Ok(result.into())
    }

    pub fn decode_value<T, P>(value: P) -> BuckyResult<T>
    where
        T: TryFrom<P>,
        <T as TryFrom<P>>::Error: std::fmt::Display,
    {
        T::try_from(value).map_err(|e| {
            let msg = format!(
                "decode value to target type failed! {} => {}, {}",
                std::any::type_name::<P>(),
                std::any::type_name::<T>(),
                e
            );

            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }

    pub fn decode_value_list<T, P>(list: impl Into<Vec<P>>) -> BuckyResult<Vec<T>>
    where
        T: TryFrom<P>,
        <T as TryFrom<P>>::Error: std::fmt::Display,
    {
        let list = list.into();
        let mut result = Vec::with_capacity(list.len());
        for v in list {
            let item = Self::decode_value(v)?;

            result.push(item);
        }

        Ok(result)
    }

    pub fn decode_str_value<T>(value: &str) -> BuckyResult<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        T::from_str(value).map_err(|e| {
            let msg = format!(
                "decode string to target type failed! {} => {}, {}",
                value,
                std::any::type_name::<T>(),
                e
            );

            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })
    }
}

pub trait ProtobufEncode {}
pub trait ProtobufDecode {}

pub trait ProtobufTransform<T>: Sized {
    fn transform(value: T) -> BuckyResult<Self>;
}
//
// impl<T> ProtobufTransform<T> for T {
//     fn transform(value: T) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl <T: Clone> ProtobufTransform<&T> for T {
//     fn transform(value: &T) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }

impl <T, U: ProtobufTransform<T>> ProtobufTransform<Vec<T>> for Vec<U> {
    fn transform(value: Vec<T>) -> BuckyResult<Self> {
        let mut list = Vec::new();
        for item in value.into_iter() {
            list.push(ProtobufTransform::transform(item)?);
        }
        Ok(list)
    }
}

impl <'a, T: 'a, U: ProtobufTransform<&'a T>> ProtobufTransform<&'a Vec<T>> for Vec<U> {
    fn transform(value: &'a Vec<T>) -> BuckyResult<Self> {
        let mut list = Vec::new();
        for item in value.into_iter() {
            list.push(ProtobufTransform::transform(item)?);
        }
        Ok(list)
    }
}

impl <K, T, Y: ProtobufTransform<K> + std::cmp::Eq + std::hash::Hash, U: ProtobufTransform<T>> ProtobufTransform<HashMap<K, T>> for HashMap<Y, U> {
    fn transform(value: HashMap<K, T>) -> BuckyResult<Self> {
        let mut list = HashMap::new();
        for (k, t) in value.into_iter() {
            list.insert(ProtobufTransform::transform(k)?, ProtobufTransform::transform(t)?);
        }
        Ok(list)
    }
}

impl <'a, K: 'a, T: 'a, Y: ProtobufTransform<&'a K> + std::cmp::Eq + std::hash::Hash, U: ProtobufTransform<&'a T>> ProtobufTransform<&'a HashMap<K, T>> for HashMap<Y, U> {
    fn transform(value: &'a HashMap<K, T>) -> BuckyResult<Self> {
        let mut list = HashMap::new();
        for (k, t) in value.into_iter() {
            list.insert(ProtobufTransform::transform(k)?, ProtobufTransform::transform(t)?);
        }
        Ok(list)
    }
}

// impl<T: Into<T>, U> ProtobufTransform<T> for U {
//     fn transform(value: T) -> BuckyResult<Self> {
//         Ok(value.into())
//     }
// }
//
// impl<T, U: ProtobufTransform<T>> ProtobufTransform<Option<T>> for U {
//     fn transform(value: Option<T>) -> BuckyResult<Self> {
//         match value {
//             Some(value) => ProtobufTransform::transform(value),
//             None => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("transform failed. value can't None")))
//         }
//     }
// }

impl<T, U: ProtobufTransform<T>> ProtobufTransform<Option<T>> for Option<U> {
    fn transform(value: Option<T>) -> BuckyResult<Self> {
        match value {
            Some(value) => Ok(Some(ProtobufTransform::transform(value)?)),
            None => Ok(None)
        }
    }
}

impl<'a, T: 'a, U: ProtobufTransform<&'a T>> ProtobufTransform<&'a Option<T>> for Option<U> {
    fn transform(value: &'a Option<T>) -> BuckyResult<Self> {
        match value {
            Some(value) => Ok(Some(ProtobufTransform::transform(value)?)),
            None => Ok(None)
        }
    }
}

impl ProtobufTransform<i32> for i8 {
    fn transform(value: i32) -> BuckyResult<Self> {
        Ok(value as i8)
    }
}

impl ProtobufTransform<i32> for u8 {
    fn transform(value: i32) -> BuckyResult<Self> {
        Ok(value as u8)
    }
}

impl ProtobufTransform<u32> for u8 {
    fn transform(value: u32) -> BuckyResult<Self> {
        Ok(value as u8)
    }
}

impl ProtobufTransform<i32> for i16 {
    fn transform(value: i32) -> BuckyResult<Self> {
        Ok(value as i16)
    }
}

impl ProtobufTransform<i32> for u16 {
    fn transform(value: i32) -> BuckyResult<Self> {
        Ok(value as u16)
    }
}

impl ProtobufTransform<u32> for u16 {
    fn transform(value: u32) -> BuckyResult<Self> {
        Ok(value as u16)
    }
}

impl ProtobufTransform<&i32> for i16 {
    fn transform(value: &i32) -> BuckyResult<Self> {
        Ok(*value as i16)
    }
}

impl ProtobufTransform<&u32> for u16 {
    fn transform(value: &u32) -> BuckyResult<Self> {
        Ok(*value as u16)
    }
}

impl ProtobufTransform<i32> for i32 {
    fn transform(value: i32) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<i8> for i32 {
    fn transform(value: i8) -> BuckyResult<Self> {
        Ok(value as i32)
    }
}

impl ProtobufTransform<u8> for i32 {
    fn transform(value: u8) -> BuckyResult<Self> {
        Ok(value as i32)
    }
}

impl ProtobufTransform<i16> for i32 {
    fn transform(value: i16) -> BuckyResult<Self> {
        Ok(value as i32)
    }
}

impl ProtobufTransform<u16> for i32 {
    fn transform(value: u16) -> BuckyResult<Self> {
        Ok(value as i32)
    }
}

impl ProtobufTransform<&i8> for i32 {
    fn transform(value: &i8) -> BuckyResult<Self> {
        Ok(*value as i32)
    }
}

impl ProtobufTransform<&i16> for i32 {
    fn transform(value: &i16) -> BuckyResult<Self> {
        Ok(*value as i32)
    }
}

impl ProtobufTransform<&u8> for i32 {
    fn transform(value: &u8) -> BuckyResult<Self> {
        Ok(*value as i32)
    }
}

impl ProtobufTransform<&u16> for i32 {
    fn transform(value: &u16) -> BuckyResult<Self> {
        Ok(*value as i32)
    }
}

impl ProtobufTransform<u32> for u32 {
    fn transform(value: u32) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<u8> for u32 {
    fn transform(value: u8) -> BuckyResult<Self> {
        Ok(value as u32)
    }
}

impl ProtobufTransform<u16> for u32 {
    fn transform(value: u16) -> BuckyResult<Self> {
        Ok(value as u32)
    }
}

impl ProtobufTransform<&u8> for u32 {
    fn transform(value: &u8) -> BuckyResult<Self> {
        Ok(*value as u32)
    }
}

impl ProtobufTransform<&u16> for u32 {
    fn transform(value: &u16) -> BuckyResult<Self> {
        Ok(*value as u32)
    }
}

impl ProtobufTransform<&i32> for i32 {
    fn transform(value: &i32) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<&u32> for u32 {
    fn transform(value: &u32) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<i64> for i64 {
    fn transform(value: i64) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<u64> for u64 {
    fn transform(value: u64) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&i64> for i64 {
    fn transform(value: &i64) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<&u64> for u64 {
    fn transform(value: &u64) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<f32> for f32 {
    fn transform(value: f32) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&f32> for f32 {
    fn transform(value: &f32) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<f64> for f64 {
    fn transform(value: f64) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&f64> for f64 {
    fn transform(value: &f64) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<String> for String {
    fn transform(value: String) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&String> for String {
    fn transform(value: &String) -> BuckyResult<Self> {
        Ok(value.to_string())
    }
}

impl ProtobufTransform<bool> for bool {
    fn transform(value: bool) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&bool> for bool {
    fn transform(value: &bool) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<u8> for u8 {
    fn transform(value: u8) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&u8> for u8 {
    fn transform(value: &u8) -> BuckyResult<Self> {
        Ok(*value)
    }
}

impl ProtobufTransform<u16> for u16 {
    fn transform(value: u16) -> BuckyResult<Self> {
        Ok(value)
    }
}

impl ProtobufTransform<&u16> for u16 {
    fn transform(value: &u16) -> BuckyResult<Self> {
        Ok(*value)
    }
}

// impl ProtobufTransform<Vec<i8>> for Vec<i8> {
//     fn transform(value: Vec<i8>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<i8>> for Vec<i8> {
//     fn transform(value: &Vec<i8>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<u8>> for Vec<u8> {
//     fn transform(value: Vec<u8>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<u8>> for Vec<u8> {
//     fn transform(value: &Vec<u8>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<i16>> for Vec<i16> {
//     fn transform(value: Vec<i16>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<i16>> for Vec<i16> {
//     fn transform(value: &Vec<i16>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<u16>> for Vec<u16> {
//     fn transform(value: Vec<u16>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<u16>> for Vec<u16> {
//     fn transform(value: &Vec<u16>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<i32>> for Vec<i32> {
//     fn transform(value: Vec<i32>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<i32>> for Vec<i32> {
//     fn transform(value: &Vec<i32>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<u32>> for Vec<u32> {
//     fn transform(value: Vec<u32>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<u32>> for Vec<u32> {
//     fn transform(value: &Vec<u32>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
//
// impl ProtobufTransform<Vec<i64>> for Vec<i64> {
//     fn transform(value: Vec<i64>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<i64>> for Vec<i64> {
//     fn transform(value: &Vec<i64>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }
//
// impl ProtobufTransform<Vec<u64>> for Vec<u64> {
//     fn transform(value: Vec<u64>) -> BuckyResult<Self> {
//         Ok(value)
//     }
// }
//
// impl ProtobufTransform<&Vec<u64>> for Vec<u64> {
//     fn transform(value: &Vec<u64>) -> BuckyResult<Self> {
//         Ok(value.clone())
//     }
// }

/*
impl<T: ::protobuf::Message> RawEncode for T {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size = self.compute_size() as usize;
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let mut stream = ::protobuf::CodedOutputStream::bytes(buf);
        self.write_to(&mut stream).map_err(|e| {
            let msg = format!("write protobuf::Message to stream error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::OutOfLimit, msg)
        })?;

        let stream_exposed: stream_pos_retreve_helper::CodedOutputStream = unsafe {
            std::mem::transmute(stream)
        };

        Ok(&mut buf[stream_exposed.position..])
    }
}

impl<'de, T: ::protobuf::Message> RawDecode<'de> for T {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        todo!();
    }
}
*/

/*
为某个struct实现基于protobuf的rawcodec编解码，需要满足如下条件
1. 为结构体{name}定义同名的proto文件，并编译成对应的rust放到相同工程，并且可以通过protos::{name}来引用
2. 定义{name}和protos::{name}互转的TryFrom，签名如下：
    impl TryFrom<&{name}> for protos::{name}} {
        type Error = BuckyError;

        fn try_from(value: &{name}) -> BuckyResult<Self> {
            ......
        }
    }

    impl TryFrom<protos::{name}> for {name} {
        type Error = BuckyError;

        fn try_from(value: protos::{name}) -> BuckyResult<Self> {
            ......
        }
    }
*/

#[macro_export]
macro_rules! impl_default_protobuf_raw_codec {
    ($name:ty, $proto_name:ty) => {
        impl cyfs_base::RawEncode for $name {
            fn raw_measure(
                &self,
                purpose: &Option<cyfs_base::RawEncodePurpose>,
            ) -> cyfs_base::BuckyResult<usize> {
                // info!("desc content measure");
                cyfs_base::ProtobufCodecHelper::raw_measure::<$name, $proto_name>(&self, purpose)
            }
            fn raw_encode<'a>(
                &self,
                buf: &'a mut [u8],
                purpose: &Option<cyfs_base::RawEncodePurpose>,
            ) -> cyfs_base::BuckyResult<&'a mut [u8]> {
                // info!("desc content encode");
                cyfs_base::ProtobufCodecHelper::raw_encode::<$name, $proto_name>(self, buf, purpose)
            }
        }
        impl<'de> cyfs_base::RawDecode<'de> for $name {
            fn raw_decode(buf: &'de [u8]) -> cyfs_base::BuckyResult<(Self, &'de [u8])> {
                // info!("desc content decode");
                cyfs_base::ProtobufCodecHelper::raw_decode::<$name, $proto_name>(buf)
            }
        }
    };

    ($name:ident) => {
        cyfs_base::impl_default_protobuf_raw_codec!($name, protos::$name);
    };
}

// cyfs_base工程内部使用
#[macro_export]
macro_rules! inner_impl_default_protobuf_raw_codec {
    ($name:ty, $proto_name:ty) => {
        impl crate::RawEncode for $name {
            fn raw_measure(
                &self,
                purpose: &Option<crate::RawEncodePurpose>,
            ) -> crate::BuckyResult<usize> {
                // info!("desc content measure");
                crate::ProtobufCodecHelper::raw_measure::<$name, $proto_name>(&self, purpose)
            }
            fn raw_encode<'a>(
                &self,
                buf: &'a mut [u8],
                purpose: &Option<crate::RawEncodePurpose>,
            ) -> crate::BuckyResult<&'a mut [u8]> {
                // info!("desc content encode");
                crate::ProtobufCodecHelper::raw_encode::<$name, $proto_name>(self, buf, purpose)
            }
        }
        impl<'de> crate::RawDecode<'de> for $name {
            fn raw_decode(buf: &'de [u8]) -> crate::BuckyResult<(Self, &'de [u8])> {
                // info!("desc content decode");
                crate::ProtobufCodecHelper::raw_decode::<$name, $proto_name>(buf)
            }
        }
    };

    ($name:ident) => {
        crate::inner_impl_default_protobuf_raw_codec!($name, protos::$name);
    };
}

// 用以为空结构体实现基于protobuf的编解码
#[macro_export]
macro_rules! mod_impl_empty_protobuf_raw_codec {
    ($m:ident, $name:ty, $proto_name:ty) => {
        impl $m::RawEncode for $name {
            fn raw_measure(
                &self,
                _purpose: &Option<$m::RawEncodePurpose>,
            ) -> $m::BuckyResult<usize> {
                Ok(0)
            }
            fn raw_encode<'a>(
                &self,
                buf: &'a mut [u8],
                _purpose: &Option<$m::RawEncodePurpose>,
            ) -> $m::BuckyResult<&'a mut [u8]> {
                (Ok(buf))
            }
        }
        impl<'de> $m::RawDecode<'de> for $name {
            fn raw_decode(buf: &'de [u8]) -> $m::BuckyResult<(Self, &'de [u8])> {
                // info!("desc content decode");

                let (msg, buf) = $m::ProtobufMessageCodecHelper::raw_decode::<$proto_name>(buf)?;

                // 如果存在unknown fields，那么打印
                use ::protobuf::Message;
                if let Some(list) = &msg.get_unknown_fields().fields {
                    warn!("got unknown fields! count={}", list.len());
                }
                Ok((Self {}, buf))
            }
        }
    };

    ($m:ident, $name:ident) => {
        $m::mod_impl_empty_protobuf_raw_codec!($m, $name, $m::EmptyContent);
    };
}

#[macro_export]
macro_rules! impl_empty_protobuf_raw_codec {
    ($name:ty, $proto_name:ty) => {
        mod_impl_empty_protobuf_raw_codec!(cyfs_base, $name, $proto_name);
    };

    ($name:ident) => {
        mod_impl_empty_protobuf_raw_codec!(cyfs_base, $name);
    };
}

#[macro_export]
macro_rules! inner_impl_empty_protobuf_raw_codec {
    ($name:ty, $proto_name:ty) => {
        crate::mod_impl_empty_protobuf_raw_codec!(crate, $name, $proto_name);
    };

    ($name:ident) => {
        crate::mod_impl_empty_protobuf_raw_codec!(crate, $name);
    };
}

#[cfg(test)]
mod test {
    use crate::*;
    use ::protobuf::Message;
    use std::convert::TryFrom;

    // 空结构体可以使用raw_codec，也可以使用protobuf辅助宏来实现
    #[derive(Clone, Debug, RawEncode, RawDecode)]
    struct EmptyContent {}

    struct EmptyContent2 {}

    inner_impl_empty_protobuf_raw_codec!(EmptyContent2);

    struct EmptyContentV1 {
        name: Option<String>,
    }

    impl TryFrom<protos::EmptyContentV1> for EmptyContentV1 {
        type Error = BuckyError;
        fn try_from(mut value: protos::EmptyContentV1) -> BuckyResult<Self> {
            let mut ret = Self { name: None };

            if value.has_name() {
                ret.name = Some(value.take_name());
            }

            Ok(ret)
        }
    }
    impl TryFrom<&EmptyContentV1> for protos::EmptyContentV1 {
        type Error = BuckyError;
        fn try_from(value: &EmptyContentV1) -> BuckyResult<Self> {
            let mut ret = Self::new();
            if let Some(name) = &value.name {
                ret.set_name(name.to_owned());
            }

            Ok(ret)
        }
    }
    inner_impl_default_protobuf_raw_codec!(EmptyContentV1);

    #[test]
    fn test_protobuf() {
        // 新版本兼容老版本
        {
            let content = protos::EmptyContent::new();
            let size = content.compute_size();
            assert_eq!(size, 0);

            let buf = vec![0u8; 0];
            let (content_v1, _) = EmptyContentV1::raw_decode(&buf).unwrap();
            assert!(content_v1.name.is_none());
        }

        // 老版本兼容新版本
        {
            let content_v1 = EmptyContentV1 {
                name: Some("xxx".to_owned()),
            };
            let buf = content_v1.to_vec().unwrap();
            assert!(buf.len() > 0);

            // 如果是使用了默认的rawcodec，那么解码后buf长度不会变化
            // 但我们在上层object_mut_body实际没使用返回的buf，所以可以完全兼容
            let (_content, left_buf) = EmptyContent::raw_decode(&buf).unwrap();
            assert!(left_buf.len() == buf.len());

            // 如果是使用了protobuf编解码，那么就会消耗掉整个buf
            let (_content, left_buf) = EmptyContent2::raw_decode(&buf).unwrap();
            assert!(left_buf.len() == 0);
        }

        let content2 = EmptyContent {};
        let buf = content2.to_vec().unwrap();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn test_helper() {
        let mut source: u32 = u32::MAX;
        let ret = ProtobufCodecHelper::decode_value::<u8, u32>(source);
        assert!(ret.is_err());

        source = u8::MAX as u32;
        let ret = ProtobufCodecHelper::decode_value::<u8, u32>(source);
        assert!(ret.is_ok());
        assert_eq!(ret.unwrap(), u8::MAX);
    }
}
