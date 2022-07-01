use crate::codec::*;
use crate::CoreObjectType;
use cyfs_base::*;

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, hash_map};

use serde::{Serialize, Deserialize};

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct FriendListDescContent {}

impl DescContent for FriendListDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::FriendList as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FriendContent {}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::FriendListContent)]
pub struct FriendListContent {
    pub(crate) friends: HashMap<ObjectId, FriendContent>,
    pub(crate) auto_confirm: u8,
    pub(crate) auto_msg: String,
}

impl BodyContent for FriendListContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ProtobufTransform<protos::FriendListContent> for FriendListContent {
    fn transform(value: protos::FriendListContent) -> BuckyResult<Self> {
        let mut friends = HashMap::new();
        for item in value.friends {
            let id = ObjectId::clone_from_slice(item.id.as_slice());
            match friends.entry(id) {
                hash_map::Entry::Vacant(entry) => {
                    // 这里由于是空结构体，不需要解码
                    let content = FriendContent {};
                    entry.insert(content);
                },
                hash_map::Entry::Occupied(entry)=>{
                    error!("decode AppListContent source but got repeated item: {}", entry.key());
                }
            }
        }

        Ok(Self {
            friends,
            auto_confirm: value.auto_confirm as u8,
            auto_msg: value.auto_msg,
        })
    }
}
impl ProtobufTransform<&FriendListContent> for protos::FriendListContent {
    fn transform(value: &FriendListContent) -> BuckyResult<Self> {
        let mut friends = Vec::with_capacity(value.friends.len());
        for (k, _v) in &value.friends {
            let item = protos::FriendItem { id: k.to_vec()?, content: vec![] };
            friends.push(item);
        }

        Ok(Self {
            friends,
            auto_confirm: value.auto_confirm as u32,
            auto_msg: value.auto_msg.clone()
        })
    }
}

type FriendListType = NamedObjType<FriendListDescContent, FriendListContent>;
type FriendListBuilder = NamedObjectBuilder<FriendListDescContent, FriendListContent>;
type FriendListDesc = NamedObjectDesc<FriendListDescContent>;

pub type FriendListId = NamedObjectId<FriendListType>;
pub type FriendList = NamedObjectBase<FriendListType>;

pub trait FriendListObj {
    fn create(owner: ObjectId, auto_confirm: bool) -> Self;
    fn friend_list_mut(&mut self) -> &mut HashMap<ObjectId, FriendContent>;
    fn friend_list(&self) -> &HashMap<ObjectId, FriendContent>;
    fn auto_confirm(&self) -> bool;
    fn set_auto_confirm(&mut self, auto_confirm: bool);
    fn auto_msg(&self) -> &str;
    fn set_auto_msg(&mut self, msg: String);
}

impl FriendListObj for FriendList {
    fn create(owner: ObjectId, auto_confirm: bool) -> Self {
        // 加个检测
        let owner_type = owner.obj_type_code();
        if owner_type != ObjectTypeCode::People && owner_type != ObjectTypeCode::SimpleGroup {
            // 如果owner不是people或者simplegroup，给个警告
            warn!("friend list owner {} type {} not people or group!", &owner, owner_type.to_u8());
        }

        let body = FriendListContent {
            friends: HashMap::new(),
            auto_confirm: if auto_confirm { 1 } else { 0 },
            auto_msg: "".to_string(),
        };
        let desc = FriendListDescContent {};
        FriendListBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn friend_list_mut(&mut self) -> &mut HashMap<ObjectId, FriendContent> {
        &mut self.body_mut_expect("").content_mut().friends
    }

    fn friend_list(&self) -> &HashMap<ObjectId, FriendContent, RandomState> {
        &self.body_expect("").content().friends
    }

    fn auto_confirm(&self) -> bool {
        self.body_expect("").content().auto_confirm == 1
    }

    fn set_auto_confirm(&mut self, auto_confirm: bool) {
        self.body_mut_expect("").content_mut().auto_confirm = if auto_confirm { 1 } else { 0 };
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn auto_msg(&self) -> &str {
        &self.body_expect("").content().auto_msg
    }

    fn set_auto_msg(&mut self, msg: String) {
        self.body_mut_expect("").content_mut().auto_msg = msg;
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }
}
#[cfg(test)]
mod test {
    use cyfs_base::*;
    use crate::*;

    #[test]
    fn test() {
        let secret1 = PrivateKey::generate_rsa(1024).unwrap();
        let secret2 = PrivateKey::generate_rsa(1024).unwrap();
        let people1 = People::new(None, vec![], secret1.public(), None, None, None).build();
        let people1_id = people1.desc().calculate_id();
        let people2 = People::new(None, vec![], secret2.public(), None, None, None).build();
        let people2_id = people2.desc().calculate_id();

        let mut friend_list = FriendList::create(people1_id, false);

        friend_list
            .friend_list_mut()
            .insert(people2_id.clone(), FriendContent {});
        let list_buf = friend_list.to_vec().unwrap();
        let (list2, _) = FriendList::raw_decode(&list_buf).unwrap();
        let (any1, _) = AnyNamedObject::raw_decode(&list_buf).unwrap();

        assert_eq!(friend_list.desc().calculate_id(), any1.calculate_id());
        assert_eq!(
            friend_list.desc().calculate_id(),
            list2.desc().calculate_id()
        );
        assert!(list2.friend_list().contains_key(&people2_id));
    }
}
