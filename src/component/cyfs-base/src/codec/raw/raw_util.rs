use crate::*;

use std::io::Read;
use std::path::Path;

pub trait FileEncoder<D> {
    fn suggest_buffer_size(&self) -> BuckyResult<usize>;
    fn encode<'a>(&self, buf: &'a mut [u8], is_compress: bool) -> BuckyResult<&'a mut [u8]>;

    fn encode_to_writer(
        &self,
        mut writer: impl std::io::Write,
        is_compress: bool,
    ) -> BuckyResult<usize> {
        let len = self.suggest_buffer_size()?;
        let mut buf = Vec::with_capacity(len);
        buf.resize(len, 0);

        let rest = self.encode(buf.as_mut_slice(), is_compress)?;

        let encode_len = len - rest.len();
        match writer.write(&buf[..encode_len]) {
            Ok(_n) => Ok(encode_len),
            Err(e) => Err(BuckyError::from(e)),
        }
    }

    fn encode_to_file(&self, file: &Path, is_compress: bool) -> BuckyResult<usize> {
        match std::fs::File::create(file) {
            Ok(file) => self.encode_to_writer(file, is_compress),
            Err(e) => Err(BuckyError::from(e)),
        }
    }

    fn encode_to_vec(&self, is_compress: bool) -> BuckyResult<Vec<u8>> {
        let len = self.suggest_buffer_size()?;
        let mut buf = Vec::with_capacity(len);
        buf.resize(len, 0);
        self.encode(buf.as_mut_slice(), is_compress)?;
        Ok(buf)
    }
}

impl<D> FileEncoder<D> for D
where
    D: RawEncode,
{
    fn suggest_buffer_size(&self) -> BuckyResult<usize> {
        self.raw_measure(&None)
    }
    fn encode<'a>(&self, buf: &'a mut [u8], _is_compress: bool) -> BuckyResult<&'a mut [u8]> {
        self.raw_encode(buf, &None)
    }
}

pub trait FileDecoder<'de>: Sized {
    fn decode_from_file(file: &Path, buf: &'de mut Vec<u8>) -> BuckyResult<(Self, usize)>;
}

impl<'de, D> FileDecoder<'de> for D
where
    D: RawDecode<'de>,
{
    fn decode_from_file(file: &Path, buf: &'de mut Vec<u8>) -> BuckyResult<(Self, usize)> {
        match std::fs::File::open(file) {
            Ok(mut file) => {
                // let mut buf = Vec::<u8>::new();
                if let Err(e) = file.read_to_end(buf) {
                    return Err(BuckyError::from(e));
                }
                let len = buf.len();
                let (obj, buf) = D::raw_decode(buf.as_slice())?;
                let size = len - buf.len();
                Ok((obj, size))
            }
            Err(e) => Err(BuckyError::from(e)),
        }
    }
}

pub trait RawConvertTo<O> {
    fn to_vec(&self) -> BuckyResult<Vec<u8>>;
    fn to_hex(&self) -> BuckyResult<String>;
}

pub trait RawFrom<'de, O> {
    fn clone_from_slice(buf: &'de [u8]) -> BuckyResult<O>;
    fn clone_from_hex(hex_str: &str, buf: &'de mut Vec<u8>) -> BuckyResult<O>;
}

impl<T> RawConvertTo<T> for T
where
    T: RawEncode,
{
    fn to_vec(&self) -> BuckyResult<Vec<u8>> {
        self.raw_encode_to_buffer()
    }

    fn to_hex(&self) -> BuckyResult<String> {
        let buf = self.to_vec()?;
        Ok(hex::encode(buf))
    }
}

impl<'de, O> RawFrom<'de, O> for O
where
    O: RawDecode<'de>,
{
    fn clone_from_slice(buf: &'de [u8]) -> BuckyResult<O> {
        let (t, _buf) = O::raw_decode(buf)?;

        // println!("buffer_len:{}", buf.len());
        // assert_eq!(_buf.len(),0);
        Ok(t)
    }

    fn clone_from_hex(hex_str: &str, buf: &'de mut Vec<u8>) -> BuckyResult<O> {
        let buf_size = hex_str.len() / 2;
        buf.resize(buf_size, 0);
        hex::decode_to_slice(hex_str, buf)?;

        let (t, _buf) = O::raw_decode(buf)?;

        Ok(t)
    }
}
