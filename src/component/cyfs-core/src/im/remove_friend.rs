use crate::CoreObjectType;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::RemoveFriendDescContent)]
pub struct RemoveFriendDescContent {
    to: PeopleId,
}

impl DescContent for RemoveFriendDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::RemoveFriend as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

type RemoveFriendType = NamedObjType<RemoveFriendDescContent, EmptyProtobufBodyContent>;
type RemoveFriendBuilder = NamedObjectBuilder<RemoveFriendDescContent, EmptyProtobufBodyContent>;

pub type RemoveFriendId = NamedObjectId<RemoveFriendType>;
pub type RemoveFriend = NamedObjectBase<RemoveFriendType>;

impl RemoveFriendDescContent {
    pub fn new(to: PeopleId) -> Self {
        Self { to }
    }
}

pub trait RemoveFriendObject {
    fn create(owner: PeopleId, author: ObjectId, to: PeopleId) -> Self;
    fn to(&self) -> &PeopleId;
}

impl RemoveFriendObject for RemoveFriend {
    fn create(owner: PeopleId, author: ObjectId, to: PeopleId) -> Self {
        let desc = RemoveFriendDescContent::new(to);
        RemoveFriendBuilder::new(desc, EmptyProtobufBodyContent::default())
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
    use super::{RemoveFriend, RemoveFriendObject};
    use cyfs_base::{NamedObject, ObjectDesc, People, PrivateKey, RawConvertTo, RawDecode};

    #[test]
    fn test() {
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let people1 = People::new(None, vec![], secret1.public(), None, None, None)
            .build()
            .desc()
            .people_id();
        let _people2 = People::new(None, vec![], secret1.public(), None, None, None)
            .build()
            .desc()
            .people_id();

        let add1 = RemoveFriend::create(
            people1.clone(),
            people1.object_id().to_owned(),
            people1.clone(),
        );
        let buf = add1.to_vec().unwrap();
        let (add2, _) = RemoveFriend::raw_decode(&buf).unwrap();
        assert_eq!(add1.desc().calculate_id(), add2.desc().calculate_id())
    }
}
