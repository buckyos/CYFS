use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, DescContent, EmptyBodyContent, NamedObjType,
    NamedObject, NamedObjectBase, NamedObjectBuilder, NamedObjectId, RawDecode, RawEncode,
    SubDescNone, OBJECT_CONTENT_CODEC_FORMAT_RAW,
};

use crate::{CoreObjectType, HotstuffBlockQC, HotstuffTimeout};

#[derive(Clone)]
pub enum GroupQuorumCertificateDescContent {
    QC(HotstuffBlockQC),
    TC(HotstuffTimeout),
}

impl DescContent for GroupQuorumCertificateDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::GroupQuorumCertificate as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_RAW
    }

    fn debug_info() -> String {
        String::from("GroupQuorumCertificateDescContent")
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

pub type GroupQuorumCertificateType =
    NamedObjType<GroupQuorumCertificateDescContent, EmptyBodyContent>;
pub type GroupQuorumCertificateBuilder =
    NamedObjectBuilder<GroupQuorumCertificateDescContent, EmptyBodyContent>;

pub type GroupQuorumCertificateId = NamedObjectId<GroupQuorumCertificateType>;
pub type GroupQuorumCertificate = NamedObjectBase<GroupQuorumCertificateType>;

pub trait GroupQuorumCertificateObject {
    fn quorum_round(&self) -> u64;
}

impl GroupQuorumCertificateObject for GroupQuorumCertificate {
    fn quorum_round(&self) -> u64 {
        match self.desc().content() {
            GroupQuorumCertificateDescContent::QC(qc) => qc.round,
            GroupQuorumCertificateDescContent::TC(tc) => tc.round,
        }
    }
}

impl RawEncode for GroupQuorumCertificateDescContent {
    fn raw_measure(
        &self,
        purpose: &Option<cyfs_base::RawEncodePurpose>,
    ) -> cyfs_base::BuckyResult<usize> {
        let len = match self {
            GroupQuorumCertificateDescContent::QC(qc) => qc.raw_measure(purpose)?,
            GroupQuorumCertificateDescContent::TC(tc) => tc.raw_measure(purpose)?,
        };

        Ok(len + 1)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<cyfs_base::RawEncodePurpose>,
    ) -> cyfs_base::BuckyResult<&'a mut [u8]> {
        match self {
            GroupQuorumCertificateDescContent::QC(qc) => {
                buf[0] = 0;
                let buf = &mut buf[1..];
                qc.raw_encode(buf, purpose)
            }
            GroupQuorumCertificateDescContent::TC(tc) => {
                buf[0] = 1;
                let buf = &mut buf[1..];
                tc.raw_encode(buf, purpose)
            }
        }
    }
}

impl<'de> RawDecode<'de> for GroupQuorumCertificateDescContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let obj_type = buf[0];
        let buf = &buf[1..];
        match obj_type {
            0 => {
                let (qc, remain) = HotstuffBlockQC::raw_decode(buf)?;
                Ok((GroupQuorumCertificateDescContent::QC(qc), remain))
            }
            1 => {
                let (qc, remain) = HotstuffTimeout::raw_decode(buf)?;
                Ok((GroupQuorumCertificateDescContent::TC(qc), remain))
            }
            _ => Err(BuckyError::new(BuckyErrorCode::Unknown, "unknown qc")),
        }
    }
}

impl From<HotstuffBlockQC> for GroupQuorumCertificate {
    fn from(qc: HotstuffBlockQC) -> Self {
        let desc = GroupQuorumCertificateDescContent::QC(qc);
        GroupQuorumCertificateBuilder::new(desc, EmptyBodyContent).build()
    }
}

impl From<HotstuffTimeout> for GroupQuorumCertificate {
    fn from(tc: HotstuffTimeout) -> Self {
        let desc = GroupQuorumCertificateDescContent::TC(tc);
        GroupQuorumCertificateBuilder::new(desc, EmptyBodyContent).build()
    }
}

impl TryInto<HotstuffBlockQC> for GroupQuorumCertificate {
    type Error = BuckyError;

    fn try_into(self) -> Result<HotstuffBlockQC, Self::Error> {
        match self.into_desc().into_content() {
            GroupQuorumCertificateDescContent::QC(qc) => Ok(qc),
            GroupQuorumCertificateDescContent::TC(_) => {
                Err(BuckyError::new(BuckyErrorCode::Unmatch, "is tc, expect qc"))
            }
        }
    }
}

impl TryInto<HotstuffTimeout> for GroupQuorumCertificate {
    type Error = BuckyError;

    fn try_into(self) -> Result<HotstuffTimeout, Self::Error> {
        match self.into_desc().into_content() {
            GroupQuorumCertificateDescContent::TC(tc) => Ok(tc),
            GroupQuorumCertificateDescContent::QC(_) => {
                Err(BuckyError::new(BuckyErrorCode::Unmatch, "is qc, expect tc"))
            }
        }
    }
}
