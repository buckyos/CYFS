use crate::types::*;
use cyfs_base::*;
use std::convert::TryFrom;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum SnServiceGrade {
    None = 0,
    Discard = 1,
    Passable = 2,
    Normal = 3,
    Fine = 4,
    Wonderfull = 5,
}

impl SnServiceGrade {
    pub fn is_accept(&self) -> bool {
        *self >= SnServiceGrade::Passable
    }
    pub fn is_refuse(&self) -> bool {
        !self.is_accept()
    }
}

impl TryFrom<u8> for SnServiceGrade {
    type Error = BuckyError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Discard),
            2 => Ok(Self::Passable),
            3 => Ok(Self::Normal),
            4 => Ok(Self::Fine),
            5 => Ok(Self::Wonderfull),
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                "invalid SnServiceGrade value",
            )),
        }
    }
}

impl RawFixedBytes for SnServiceGrade {
    fn raw_bytes() -> Option<usize> {
        Some(1)
    }
}

impl RawEncode for SnServiceGrade {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(Self::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for encode SnServiceGrade, except={}, got={}", bytes, buf.len());
            error!("{}", msg);
            
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        buf[0] = (*self) as u8;
        Ok(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for SnServiceGrade {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for decode SnServiceGrade, except={}, got={}", bytes, buf.len());
            error!("{}", msg);
            
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        let v = Self::try_from(buf[0])?;
        Ok((v, &buf[Self::raw_bytes().unwrap()..]))
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SnServiceReceiptVersion {
    Invalid = 0,
    Current = 1,
}

impl TryFrom<u8> for SnServiceReceiptVersion {
    type Error = BuckyError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Invalid),
            1 => Ok(Self::Current),
            _ => Err(BuckyError::new(
                BuckyErrorCode::UnSupport,
                format!("unsupport version({})", v).as_str(),
            )),
        }
    }
}

impl RawFixedBytes for SnServiceReceiptVersion {
    fn raw_bytes() -> Option<usize> {
        Some(1)
    }
}

impl RawEncode for SnServiceReceiptVersion {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(Self::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for encode SnServiceReceiptVersion, except={}, got={}", bytes, buf.len());
            error!("{}", msg);
            
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        buf[0] = (*self) as u8;
        Ok(&mut buf[1..])
    }
}

impl<'de> RawDecode<'de> for SnServiceReceiptVersion {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!("not enough buffer for decode SnServiceReceiptVersion, except={}, got={}", bytes, buf.len());
            error!("{}", msg);
            
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                msg,
            ));
        }
        let v = Self::try_from(buf[0])?;
        Ok((v, &buf[Self::raw_bytes().unwrap()..]))
    }
}

struct SnServiceReceiptSignature {
    sn_peerid: DeviceId,
    receipt: SnServiceReceipt,
}

impl RawEncode for SnServiceReceiptSignature {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        let len = self.sn_peerid.raw_measure(purpose)? + self.receipt.raw_measure(purpose)?;
        Ok(len)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.sn_peerid.raw_encode(buf, purpose)?;
        self.receipt.raw_encode(buf, purpose)
    }
}

#[derive(Copy, Clone)]
pub struct SnServiceReceipt {
    pub version: SnServiceReceiptVersion,
    pub grade: SnServiceGrade,
    pub rto: u16,
    pub duration: Duration,
    pub start_time: SystemTime,
    pub ping_count: u32,
    pub ping_resp_count: u32,
    pub called_count: u32,
    pub call_peer_count: u32,
    pub connect_peer_count: u32,
    pub call_delay: u16,
}

impl SnServiceReceipt {
    pub fn sign(
        &self,
        sn_peerid: &DeviceId,
        _private_key: &PrivateKey,
    ) -> Result<Signature, BuckyError> {
        let _sig_fields = SnServiceReceiptSignature {
            sn_peerid: sn_peerid.clone(),
            receipt: self.clone(),
        };
        //FIMXE: sign
        unimplemented!()
        // Authorized::sign(&sig_fields, private_key)
    }

    pub fn verify(
        &self,
        sn_peerid: &DeviceId,
        _sign: &Signature,
        _const_info: &DeviceDesc,
    ) -> bool {
        let _sig_fields = SnServiceReceiptSignature {
            sn_peerid: sn_peerid.clone(),
            receipt: self.clone(),
        };
        //FIMXE: verify
        unimplemented!()
        //Authorized::verify(&sig_fields, sign, const_info)
    }
}

impl Default for SnServiceReceipt {
    fn default() -> Self {
        SnServiceReceipt {
            version: SnServiceReceiptVersion::Invalid,
            grade: SnServiceGrade::None,
            rto: 0,
            duration: Duration::from_millis(0),
            start_time: UNIX_EPOCH,
            ping_count: 0,
            ping_resp_count: 0,
            called_count: 0,
            call_peer_count: 0,
            connect_peer_count: 0,
            call_delay: 0,
        }
    }
}

impl RawEncode for SnServiceReceipt {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        let mut size = self.version.raw_measure(purpose)?;
        size += self.grade.raw_measure(purpose)?;
        size += self.rto.raw_measure(purpose)?;
        size += 0u32.raw_measure(purpose)?;
        size += 0u64.raw_measure(purpose)?;
        size += self.ping_count.raw_measure(purpose)?;
        size += self.ping_resp_count.raw_measure(purpose)?;
        size += self.called_count.raw_measure(purpose)?;
        size += self.call_peer_count.raw_measure(purpose)?;
        size += self.connect_peer_count.raw_measure(purpose)?;
        size += self.call_delay.raw_measure(purpose)?;
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.version.raw_encode(buf, purpose)?;
        let buf = self.grade.raw_encode(buf, purpose)?;
        let buf = self.rto.raw_encode(buf, purpose)?;
        let buf = (self.duration.as_millis() as u32).raw_encode(buf, purpose)?;
        let buf = system_time_to_bucky_time(&self.start_time).raw_encode(buf, purpose)?;
        let buf = self.ping_count.raw_encode(buf, purpose)?;
        let buf = self.ping_resp_count.raw_encode(buf, purpose)?;
        let buf = self.called_count.raw_encode(buf, purpose)?;
        let buf = self.call_peer_count.raw_encode(buf, purpose)?;
        let buf = self.connect_peer_count.raw_encode(buf, purpose)?;
        let buf = self.call_delay.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for SnServiceReceipt {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (version, buf) = SnServiceReceiptVersion::raw_decode(buf)?;
        let (grade, buf) = SnServiceGrade::raw_decode(buf)?;
        let (rto, buf) = u16::raw_decode(buf)?;
        let (duration, buf) = u32::raw_decode(buf)?;
        let duration = Duration::from_millis(duration as u64);
        let (timestamp, buf) = Timestamp::raw_decode(buf)?;
        if timestamp < MIN_BUCKY_TIME {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "invalid timestamp",
            ));
        }
        let start_time = bucky_time_to_system_time(timestamp);
        let (ping_count, buf) = u32::raw_decode(buf)?;
        let (ping_resp_count, buf) = u32::raw_decode(buf)?;
        let (called_count, buf) = u32::raw_decode(buf)?;
        let (call_peer_count, buf) = u32::raw_decode(buf)?;
        let (connect_peer_count, buf) = u32::raw_decode(buf)?;
        let (call_delay, buf) = u16::raw_decode(buf)?;
        Ok((
            SnServiceReceipt {
                version,
                grade,
                rto,
                duration,
                start_time,
                ping_count,
                ping_resp_count,
                called_count,
                call_peer_count,
                connect_peer_count,
                call_delay,
            },
            buf,
        ))
    }
}

pub struct ReceiptWithSignature(SnServiceReceipt, Signature);

impl ReceiptWithSignature {
    pub fn receipt(&self) -> &SnServiceReceipt {
        &self.0
    }

    pub fn signature(&self) -> &Signature {
        &self.1
    }
}

impl RawEncode for ReceiptWithSignature {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(self.0.raw_measure(purpose)? + self.1.raw_measure(purpose)?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let buf = self.0.raw_encode(buf, purpose)?;
        let buf = self.1.raw_encode(buf, purpose)?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for ReceiptWithSignature {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (receipt, buf) = RawDecode::raw_decode(buf)?;
        let (sig, buf) = RawDecode::raw_decode(buf)?;
        Ok((Self(receipt, sig), buf))
    }
}

impl From<(SnServiceReceipt, Signature)> for ReceiptWithSignature {
    fn from(v: (SnServiceReceipt, Signature)) -> Self {
        Self(v.0, v.1)
    }
}
