use crate::codec::protos::backup_objects as protos;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveInnerFileMeta {
    pub access: u32,             // access_string
    pub insert_time: u64,        // insert_time
    pub update_time: u64,        // update_time
    pub create_dec_id: ObjectId, // create_dec_id

    pub storage_category: NamedObjectStorageCategory, // StorageCategory
    pub context: Option<String>,                      // context
}

impl TryFrom<protos::ArchiveInnerFileMeta> for ArchiveInnerFileMeta {
    type Error = BuckyError;

    fn try_from(mut value: protos::ArchiveInnerFileMeta) -> BuckyResult<Self> {
        let create_dec_id = cyfs_base::ProtobufCodecHelper::decode_buf(value.take_create_dec_id())?;
        let storage_category = match value.storage_category {
            protos::NamedObjectStorageCategory::Storage => NamedObjectStorageCategory::Storage,
            protos::NamedObjectStorageCategory::Cache => NamedObjectStorageCategory::Cache,
        };

        Ok(Self {
            access: value.access,
            insert_time: value.insert_time,
            update_time: value.update_time,
            create_dec_id,
            storage_category,
            context: if value.has_context() {
                Some(value.take_context())
            } else {
                None
            },
        })
    }
}

impl TryFrom<&ArchiveInnerFileMeta> for protos::ArchiveInnerFileMeta {
    type Error = BuckyError;

    fn try_from(value: &ArchiveInnerFileMeta) -> BuckyResult<Self> {
        let mut ret = protos::ArchiveInnerFileMeta::new();
        ret.set_create_dec_id(value.create_dec_id.to_vec().unwrap());

        ret.set_access(value.access);
        ret.set_insert_time(value.insert_time);
        ret.set_update_time(value.update_time);

        let storage_category = match value.storage_category {
            NamedObjectStorageCategory::Storage => protos::NamedObjectStorageCategory::Storage,
            NamedObjectStorageCategory::Cache => protos::NamedObjectStorageCategory::Cache,
        };

        ret.set_storage_category(storage_category);
        if let Some(context) = &value.context {
            ret.set_context(context.clone());
        }

        Ok(ret)
    }
}

impl_default_protobuf_raw_codec!(ArchiveInnerFileMeta);

impl From<&NamedObjectMetaData> for ArchiveInnerFileMeta {
    fn from(value: &NamedObjectMetaData) -> Self {
        Self {
            access: value.access_string,
            insert_time: value.insert_time,
            update_time: value.update_time,
            create_dec_id: value.create_dec_id.clone(),
            storage_category: value.storage_category,
            context: value.context.clone(),
        }
    }
}
