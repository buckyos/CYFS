use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::GroupRPathDescContent)]
pub struct GroupRPathDescContent {
    group_id: ObjectId,
    dec_id: ObjectId,
    r_path: String,
}

impl DescContent for GroupRPathDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::GroupRPath as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("GroupRPathDescContent")
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

type GroupRPathType = NamedObjType<GroupRPathDescContent, EmptyProtobufBodyContent>;
type GroupRPathBuilder = NamedObjectBuilder<GroupRPathDescContent, EmptyProtobufBodyContent>;

pub type GroupRPathId = NamedObjectId<GroupRPathType>;
pub type GroupRPath = NamedObjectBase<GroupRPathType>;

impl GroupRPathDescContent {
    pub fn new(group_id: ObjectId, dec_id: ObjectId, r_path: String) -> Self {
        Self {
            group_id,
            dec_id,
            r_path,
        }
    }
}

pub trait GroupRPathObject {
    fn create(group_id: ObjectId, dec_id: ObjectId, r_path: String) -> Self;
    fn group_id(&self) -> &ObjectId;
    fn dec_id(&self) -> &ObjectId;
    fn r_path(&self) -> &str;
}

impl GroupRPathObject for GroupRPath {
    fn create(group_id: ObjectId, dec_id: ObjectId, r_path: String) -> Self {
        let desc = GroupRPathDescContent {
            group_id,
            dec_id,
            r_path,
        };

        GroupRPathBuilder::new(desc, EmptyProtobufBodyContent::default()).build()
    }

    fn group_id(&self) -> &ObjectId {
        &self.desc().content().group_id
    }

    fn dec_id(&self) -> &ObjectId {
        &self.desc().content().dec_id
    }

    fn r_path(&self) -> &str {
        self.desc().content().r_path.as_str()
    }
}
#[cfg(test)]
mod test {
    use super::{GroupRPath, GroupRPathObject};
    use cyfs_base::*;

    #[async_std::test]
    async fn create_group_rpath() {
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        let people1_id = people1.desc().people_id();
        let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        let _people2_id = people2.desc().people_id();

        let g1 = GroupRPath::create(
            people1_id.object_id().to_owned(),
            people1_id.object_id().to_owned(),
            people1_id.to_string(),
        );

        let buf = g1.to_vec().unwrap();
        let add2 = GroupRPath::clone_from_slice(&buf).unwrap();
        let any = AnyNamedObject::clone_from_slice(&buf).unwrap();
        assert_eq!(g1.desc().calculate_id(), add2.desc().calculate_id());
        assert_eq!(g1.desc().calculate_id(), any.calculate_id());
    }
}
