use crate::*;

use std::convert::TryFrom;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct UnionAccountDescContent {
    left: ObjectId,
    right: ObjectId,
    service_type: u8,
}

impl UnionAccountDescContent {
    pub fn new(account1: ObjectId, account2: ObjectId, service_type: u8) -> Self {
        let mut left = account1;
        let mut right = account2;
        if account1 > account2 {
            left = account2;
            right = account1;
        }

        Self {
            left,
            right,
            service_type,
        }
    }

    pub fn left(&self) -> &ObjectId {
        &self.left
    }

    pub fn right(&self) -> &ObjectId {
        &self.right
    }

    pub fn service_type(&self) -> u8 {
        self.service_type
    }
}

impl DescContent for UnionAccountDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::UnionAccount.into()
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl RawEncode for UnionAccountDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        let size =
            0 + self.left.raw_measure(purpose).map_err(|e| {
                log::error!("UnionAccountDescContent::raw_measure/left error:{}", e);
                e
            })? + self.right.raw_measure(purpose).map_err(|e| {
                log::error!("UnionAccountDescContent::raw_measure/right error:{}", e);
                e
            })? + u8::raw_bytes().unwrap();

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for UnionAccountDescContent",
            ));
        }

        let buf = self.left.raw_encode(buf, purpose).map_err(|e| {
            log::error!("UnionAccountDescContent::raw_encode/left error:{}", e);
            e
        })?;

        let buf = self.right.raw_encode(buf, purpose).map_err(|e| {
            log::error!("UnionAccountDescContent::raw_encode/right error:{}", e);
            e
        })?;

        let buf = self.service_type.raw_encode(buf, purpose).map_err(|e| {
            log::error!(
                "UnionAccountDescContent::raw_encode/service_type error:{}",
                e
            );
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for UnionAccountDescContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (left, buf) = ObjectId::raw_decode(buf).map_err(|e| {
            log::error!("UnionAccountDescContent::raw_decode/left error:{}", e);
            e
        })?;

        let (right, buf) = ObjectId::raw_decode(buf).map_err(|e| {
            log::error!("UnionAccountDescContent::raw_decode/right error:{}", e);
            e
        })?;

        let (service_type, buf) = u8::raw_decode(buf).map_err(|e| {
            log::error!(
                "UnionAccountDescContent::raw_decode/service_type error:{}",
                e
            );
            e
        })?;

        Ok((
            Self {
                left,
                right,
                service_type,
            },
            buf,
        ))
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct UnionAccountBodyContent {}

impl BodyContent for UnionAccountBodyContent {}

impl UnionAccountBodyContent {
    pub fn new() -> Self {
        Self {}
    }
}

pub type UnionAccountType = NamedObjType<UnionAccountDescContent, UnionAccountBodyContent>;
pub type UnionAccountBuilder = NamedObjectBuilder<UnionAccountDescContent, UnionAccountBodyContent>;

pub type UnionAccountDesc = NamedObjectDesc<UnionAccountDescContent>;
pub type UnionAccountId = NamedObjectId<UnionAccountType>;
pub type UnionAccount = NamedObjectBase<UnionAccountType>;

impl UnionAccountDesc {
    pub fn tx_id(&self) -> UnionAccountId {
        UnionAccountId::try_from(self.calculate_id()).unwrap()
    }
}

impl NamedObjectBase<UnionAccountType> {
    pub fn new(account1: ObjectId, account2: ObjectId, service_type: u8) -> UnionAccountBuilder {
        let desc_content = UnionAccountDescContent::new(account1, account2, service_type);

        let body_content = UnionAccountBodyContent::new();

        UnionAccountBuilder::new(desc_content, body_content)
    }
}

#[cfg(test)]
mod test {
    use crate::{ObjectId, RawConvertTo, RawFrom, UnionAccount};
    //use std::path::Path;

    #[test]
    fn union_account() {
        let account1 = ObjectId::default();

        let account2 = ObjectId::default();

        let service_type = 0; // TODO: 常量定义

        let obj = UnionAccount::new(account1, account2, service_type).build();

        // let p = Path::new("f:\\temp\\union_account.obj");
        // if p.parent().unwrap().exists() {
        //     obj.clone().encode_to_file(p, false);
        // }

        let buf = obj.to_vec().unwrap();

        let _new_obj = UnionAccount::clone_from_slice(&buf).unwrap();
    }
}
