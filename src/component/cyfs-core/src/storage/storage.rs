use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Debug, Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::StorageDescContent)]
pub struct StorageDescContent {
    id: String,
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
    fn id(&self) -> &str;

    fn value(&self) -> &Vec<u8>;
    fn value_mut(&mut self) -> &mut Vec<u8>;

    fn update_value(&mut self, value: Vec<u8>) -> bool;
    fn into_value(self) -> Vec<u8>;

    fn storage_id(&self) -> StorageId;
}

impl StorageObj for Storage {
    fn create(id: &str, value: Vec<u8>) -> Self {
        let body = StorageBodyContent { value };
        let desc = StorageDescContent { id: id.to_owned() };
        StorageBuilder::new(desc, body).no_create_time().build()
    }

    fn id(&self) -> &str {
        &self.desc().content().id
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
        let current_value = self.value_mut();

        if *current_value == value {
            return false;
        }

        *current_value = value;

        self.body_mut()
            .as_mut()
            .unwrap()
            .set_update_time(bucky_time_now());

        true
    }

    fn storage_id(&self) -> StorageId {
        self.desc().calculate_id().try_into().unwrap()
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
