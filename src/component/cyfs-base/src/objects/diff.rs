use crate::*;

use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub struct DiffDescContent {
    file_id: FileId,         // diff 本身可能很大，diff_list 放在一个独立的文件里
    diff_list: Vec<ChunkId>, // 冗余 chunk id list，便于直接读取
}

impl DiffDescContent {
    pub fn new(file_id: FileId, diff_list: Vec<ChunkId>) -> Self {
        Self { file_id, diff_list }
    }

    pub fn file_id(&self) -> &FileId {
        &self.file_id
    }

    pub fn diff_list(&self) -> &Vec<ChunkId> {
        &self.diff_list
    }
}

impl RawEncode for DiffDescContent {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let size =
            0 + self.file_id.raw_measure(purpose).map_err(|e| {
                log::error!("DiffDescContent::raw_measure/file_id error:{}", e);
                e
            })? + self.diff_list.raw_measure(purpose).map_err(|e| {
                log::error!("DiffDescContent::raw_measure/diff_list error:{}", e);
                e
            })?;
        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for DiffDescContent",
            ));
        }

        let buf = self.file_id.raw_encode(buf, purpose).map_err(|e| {
            log::error!("DiffDescContent::raw_encode/file_id error:{}", e);
            e
        })?;

        let buf = self.diff_list.raw_encode(buf, purpose).map_err(|e| {
            log::error!("DiffDescContent::raw_encode/diff_list error:{}", e);
            e
        })?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for DiffDescContent {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (file_id, buf) = FileId::raw_decode(buf).map_err(|e| {
            log::error!("DiffDescContent::raw_decode/file_id error:{}", e);
            e
        })?;

        let (diff_list, buf) = Vec::<ChunkId>::raw_decode(buf).map_err(|e| {
            log::error!("DiffDescContent::raw_decode/diff_list error:{}", e);
            e
        })?;

        Ok((Self { file_id, diff_list }, buf))
    }
}

impl DescContent for DiffDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Diff.into()
    }

    fn debug_info() -> String {
        String::from("DiffDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct DiffBodyContent {
    // ignore
}

impl BodyContent for DiffBodyContent {}

pub type DiffType = NamedObjType<DiffDescContent, DiffBodyContent>;
pub type DiffBuilder = NamedObjectBuilder<DiffDescContent, DiffBodyContent>;

pub type DiffDesc = NamedObjectDesc<DiffDescContent>;
pub type DiffId = NamedObjectId<DiffType>;
pub type Diff = NamedObjectBase<DiffType>;

impl DiffDesc {
    pub fn diff_id(&self) -> DiffId {
        DiffId::try_from(self.calculate_id()).unwrap()
    }
}

impl Diff {
    pub fn new(file_id: FileId, diff_list: Vec<ChunkId>) -> DiffBuilder {
        let desc_content = DiffDescContent::new(file_id, diff_list);
        let body_content = DiffBodyContent {};
        DiffBuilder::new(desc_content, body_content)
    }
}

#[cfg(test)]
mod test {
    use crate::{ChunkId, Diff, FileId, RawConvertTo, RawFrom};

    #[test]
    fn diff() {
        let action = Diff::new(FileId::default(), vec![ChunkId::default()]).build();

        let buf = action.to_vec().unwrap();
        let _obj = Diff::clone_from_slice(&buf).unwrap();
    }
}
