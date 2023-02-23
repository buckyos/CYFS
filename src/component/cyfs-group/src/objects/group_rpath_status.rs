use std::collections::HashMap;

use cyfs_base::*;
use cyfs_core::{GroupConsensusBlockDesc, HotstuffBlockQC};
use cyfs_lib::NONObjectInfo;
use prost::Message;

#[derive(Clone)]
pub struct GroupRPathStatus {
    pub block_desc: GroupConsensusBlockDesc,
    pub certificate: HotstuffBlockQC,
    pub status_map: HashMap<ObjectId, NONObjectInfo>,
}

impl RawEncode for GroupRPathStatus {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let block_desc = self.block_desc.to_vec()?;
        let certificate = self.certificate.to_vec()?;
        let mut status_list = vec![];
        for (_, obj) in self.status_map.iter() {
            status_list.push(obj.to_vec()?);
        }

        let proto = crate::protos::GroupRPathStatus {
            block_desc,
            certificate,
            status_list,
        };

        Ok(proto.encoded_len())
    }

    fn raw_encode<'a>(
        &self,
        mut buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let block_desc = self.block_desc.to_vec()?;
        let certificate = self.certificate.to_vec()?;
        let mut status_list = vec![];
        for (_, obj) in self.status_map.iter() {
            status_list.push(obj.to_vec()?);
        }

        let proto = crate::protos::GroupRPathStatus {
            block_desc,
            certificate,
            status_list,
        };

        proto.encode_raw(&mut buf);

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for GroupRPathStatus {
    fn raw_decode(mut buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let proto = crate::protos::GroupRPathStatus::decode(&mut buf).map_err(|err| {
            let msg = format!("decode proto-buf for GroupRPathStatus failed {:?}", err);
            log::error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        let (block_desc, remain) =
            GroupConsensusBlockDesc::raw_decode(proto.block_desc.as_slice())?;
        assert_eq!(remain.len(), 0);
        let (certificate, remain) = HotstuffBlockQC::raw_decode(proto.certificate.as_slice())?;
        assert_eq!(remain.len(), 0);
        let mut status_map = HashMap::new();
        for obj_buf in proto.status_list.iter() {
            let (status, remain) = NONObjectInfo::raw_decode(obj_buf.as_slice())?;
            assert_eq!(remain.len(), 0);
            status_map.insert(status.object_id, status);
        }

        Ok((
            Self {
                block_desc,
                certificate,
                status_map,
            },
            buf,
        ))
    }
}

#[cfg(test)]
mod test {}
