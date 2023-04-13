use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Debug, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::StorageDescContent)]
pub struct StorageDescContent {
    id: String,
    hash: Option<HashValue>,
}

impl DescContent for StorageDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::Storage as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::StorageBodyContent)]
pub struct StorageBodyContent {
    pub(crate) value: Vec<u8>,
    freedom_attachment: Option<Vec<u8>>,
}

impl BodyContent for StorageBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type StorageType = NamedObjType<StorageDescContent, StorageBodyContent>;
type StorageBuilder = NamedObjectBuilder<StorageDescContent, StorageBodyContent>;
type StorageDesc = NamedObjectDesc<StorageDescContent>;

pub type StorageId = NamedObjectId<StorageType>;
pub type Storage = NamedObjectBase<StorageType>;

pub trait StorageObj {
    fn create(id: &str, value: Vec<u8>) -> Self;
    fn create_with_hash(id: &str, value: Vec<u8>) -> Self;
    fn create_with_hash_and_freedom(
        id: &str,
        value: Vec<u8>,
        freedom_attachment: Option<Vec<u8>>,
    ) -> Self;

    fn id(&self) -> &str;
    fn hash(&self) -> &Option<HashValue>;

    fn value(&self) -> &Vec<u8>;
    fn value_mut(&mut self) -> &mut Vec<u8>;

    fn update_value(&mut self, value: Vec<u8>) -> bool;
    fn into_value(self) -> Vec<u8>;

    fn freedom_attachment(&self) -> &Option<Vec<u8>>;
    fn freedom_attachment_mut(&mut self) -> &mut Option<Vec<u8>>;
    fn into_value_freedom(self) -> (Vec<u8>, Option<Vec<u8>>);

    fn storage_id(&self) -> StorageId;

    fn check_hash(&self) -> Option<bool>;
}

impl StorageObj for Storage {
    fn create(id: &str, value: Vec<u8>) -> Self {
        let body = StorageBodyContent {
            value,
            freedom_attachment: None,
        };
        let desc = StorageDescContent {
            id: id.to_owned(),
            hash: None,
        };
        StorageBuilder::new(desc, body).no_create_time().build()
    }

    fn create_with_hash(id: &str, value: Vec<u8>) -> Self {
        let desc = StorageDescContent {
            id: id.to_owned(),
            hash: Some(hash_data(&value)),
        };
        let body = StorageBodyContent {
            value,
            freedom_attachment: None,
        };
        StorageBuilder::new(desc, body).no_create_time().build()
    }

    fn create_with_hash_and_freedom(
        id: &str,
        value: Vec<u8>,
        freedom_attachment: Option<Vec<u8>>,
    ) -> Self {
        let mut obj = Self::create_with_hash(id, value);
        obj.body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .freedom_attachment = freedom_attachment;
        obj
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn hash(&self) -> &Option<HashValue> {
        &self.desc().content().hash
    }

    fn value(&self) -> &Vec<u8> {
        &self.body().as_ref().unwrap().content().value
    }

    fn value_mut(&mut self) -> &mut Vec<u8> {
        &mut self.body_mut().as_mut().unwrap().content_mut().value
    }

    fn into_value(self) -> Vec<u8> {
        self.into_body().unwrap().into_content().value
    }

    fn update_value(&mut self, value: Vec<u8>) -> bool {
        let need_hash = self.desc().content().hash.is_some();

        let mut hash = None;
        {
            let current_value = self.value_mut();

            if *current_value == value {
                return false;
            }

            if need_hash {
                hash = Some(hash_data(&value));
            }

            *current_value = value;
        }

        if hash.is_some() {
            self.desc_mut().content_mut().hash = hash;
        }

        self.body_mut()
            .as_mut()
            .unwrap()
            .set_update_time(bucky_time_now());

        true
    }

    fn freedom_attachment(&self) -> &Option<Vec<u8>> {
        &self.body().as_ref().unwrap().content().freedom_attachment
    }

    fn freedom_attachment_mut(&mut self) -> &mut Option<Vec<u8>> {
        &mut self
            .body_mut()
            .as_mut()
            .unwrap()
            .content_mut()
            .freedom_attachment
    }

    fn into_value_freedom(mut self) -> (Vec<u8>, Option<Vec<u8>>) {
        let body = &mut self.body_mut().as_mut().unwrap().content_mut();
        let mut value = vec![];
        let mut attachment = None;
        std::mem::swap(&mut value, &mut body.value);
        std::mem::swap(&mut attachment, &mut body.freedom_attachment);
        (value, attachment)
    }

    fn storage_id(&self) -> StorageId {
        self.desc().calculate_id().try_into().unwrap()
    }

    fn check_hash(&self) -> Option<bool> {
        self.desc().content().hash.as_ref().map(|hash| {
            hash == &hash_data(self.body().as_ref().unwrap().content().value.as_slice())
        })
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;

    #[test]
    fn test() {
        let id = "test_storage";
        let s = "self.desc().calculate_id().try_into().unwrap()".to_owned();
        let value = s.to_vec().unwrap();
        let storage_obj = Storage::create(id, value.clone());
        let storage_id = storage_obj.desc().calculate_id();
        let buf = storage_obj.to_vec().unwrap();

        let storage_obj2 = Storage::clone_from_slice(&buf).unwrap();
        assert_eq!(storage_id, storage_obj2.desc().calculate_id());
        assert_eq!(storage_obj.id(), storage_obj2.id());
        assert_eq!(*storage_obj.value(), *storage_obj2.value());

        let (any, left_buf) = AnyNamedObject::raw_decode(&buf).unwrap();
        assert_eq!(left_buf.len(), 0);
        info!("any id={}", any.calculate_id());
        assert_eq!(storage_id, any.calculate_id());

        let buf2 = any.to_vec().unwrap();
        assert_eq!(buf.len(), buf2.len());
        assert_eq!(buf, buf2);
    }
}
