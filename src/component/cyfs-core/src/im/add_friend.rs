use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AddFriendDescContent)]
pub struct AddFriendDescContent {
    to: PeopleId,
}

impl DescContent for AddFriendDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::AddFriend as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("AddFriendDescContent")
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

type AddFriendType = NamedObjType<AddFriendDescContent, EmptyProtobufBodyContent>;
type AddFriendBuilder = NamedObjectBuilder<AddFriendDescContent, EmptyProtobufBodyContent>;

pub type AddFriendId = NamedObjectId<AddFriendType>;
pub type AddFriend = NamedObjectBase<AddFriendType>;

impl AddFriendDescContent {
    pub fn new(to: PeopleId) -> Self {
        Self { to }
    }
}

pub trait AddFriendObject {
    fn create(owner: PeopleId, author: ObjectId, to: PeopleId) -> Self;
    fn to(&self) -> &PeopleId;
}

impl AddFriendObject for AddFriend {
    fn create(owner: PeopleId, author: ObjectId, to: PeopleId) -> Self {
        let desc = AddFriendDescContent { to };

        AddFriendBuilder::new(desc, EmptyProtobufBodyContent::default())
            .owner(owner.into())
            .author(author)
            .build()
    }

    fn to(&self) -> &PeopleId {
        &self.desc().content().to
    }
}
#[cfg(test)]
mod test {
    use super::{AddFriend, AddFriendObject};
    use cyfs_base::*;

    #[async_std::test]
    async fn add_friend() {
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        let people1_id = people1.desc().people_id();
        let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        let _people2_id = people2.desc().people_id();

        let mut add1 = AddFriend::create(
            people1_id.clone(),
            people1_id.object_id().to_owned(),
            people1_id.clone(),
        );

        let signer = RsaCPUObjectSigner::new(people1.desc().public_key().clone(), secret1);
        let ret = sign_and_push_named_object_body(&signer, &mut add1, &SignatureSource::RefIndex(0)).await;
        assert!(ret.is_ok());

        let buf = add1.to_vec().unwrap();
        let add2 = AddFriend::clone_from_slice(&buf).unwrap();
        let any = AnyNamedObject::clone_from_slice(&buf).unwrap();
        assert_eq!(add1.desc().calculate_id(), add2.desc().calculate_id());
        assert_eq!(add1.desc().calculate_id(), any.calculate_id());
    }
}
