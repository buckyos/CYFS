use crate::*;


#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ObjectLink {
    pub obj_id: ObjectId,
    pub obj_owner: Option<ObjectId>,
}

impl RawEncode for ObjectLink {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(ObjectId::raw_bytes().unwrap()
            + self.obj_owner.raw_measure(purpose).map_err(|e| {
                log::error!("ObjectId::raw_measure error:{}", e);
                e
            })?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = self.raw_measure(purpose).unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode ObjectLink, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let buf = self.obj_id.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectId::raw_encode/obj_id error:{}", e);
            e
        })?;

        let buf = self.obj_owner.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectId::raw_encode/obj_owner error:{}", e);
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for ObjectLink {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (obj_id, buf) = ObjectId::raw_decode(buf).map_err(|e| {
            log::error!("ObjectId::raw_decode/obj_id error:{}", e);
            e
        })?;

        let (obj_owner, buf) = Option::<ObjectId>::raw_decode(buf).map_err(|e| {
            log::error!("ObjectId::raw_decode/obj_owner error:{}", e);
            e
        })?;

        Ok((Self { obj_id, obj_owner }, buf))
    }
}