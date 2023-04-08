use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, Group, NamedObject, ObjectDesc, RawDecode, RawEncode,
};

use crate::{Storage, StorageObj};

pub trait GroupBlob: Sized {
    fn to_blob(&self) -> Storage;

    fn from_blob(blob: &Storage) -> BuckyResult<Self>;

    fn blob_storage_id(&self) -> String;
}

impl GroupBlob for Group {
    fn to_blob(&self) -> Storage {
        let len = self.raw_measure(&None).unwrap();
        let mut buf = vec![0u8; len];
        let remain = self.raw_encode(buf.as_mut_slice(), &None).unwrap();
        assert_eq!(remain.len(), 0);
        Storage::create_with_hash(self.blob_storage_id().as_str(), buf)
    }

    fn from_blob(blob: &Storage) -> BuckyResult<Self> {
        let group_buf = blob.value();
        let (group, remain) = Group::raw_decode(group_buf.as_slice())?;
        assert_eq!(remain.len(), 0);

        let expected_id = group.blob_storage_id();

        if blob.id() != expected_id {
            return Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("unknown storage, expect {}, got {}", expected_id, blob.id()),
            ));
        }

        Ok(group)
    }

    fn blob_storage_id(&self) -> String {
        format!("group@{}@{}", self.desc().object_id(), self.version())
    }
}
