use cyfs_base::*;

#[derive(Debug, Clone)]
pub enum PieceSessionType {
    Unknown,
    Stream(u32), 
    RaptorA(u32),  
    RaptorB(u32), 
    // RaptorN(u32)
} 

impl RawEncode for PieceSessionType {
    fn raw_measure(&self, _: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            Self::Unknown => Ok(u8::raw_bytes().unwrap()), 
            _ => Ok(u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap())
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Self::Unknown => 0u8.raw_encode(buf, purpose), 
            Self::Stream(start) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            },
            Self::RaptorA(start) => {
                let buf = 2u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            },
            Self::RaptorB(start) => {
                let buf = 3u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            }
        }
    }
}


impl<'de> RawDecode<'de> for PieceSessionType {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (type_code, buf) = u8::raw_decode(buf)?;
        match type_code {
            0u8 => Ok((Self::Unknown, buf)), 
            1u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::Stream(start), buf))
            },
            2u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::RaptorA(start), buf))
            },
            3u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::RaptorB(start), buf))
            },
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid type code"))
        }
    }
}

