use crate::*;
use cyfs_base::*;

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStateObjectMetaItem {
    // Object dynamic selector
    pub selector: String,

    // Access value
    pub access: GlobalStatePathGroupAccess,

    // Object referer's depth, default is 1
    pub depth: Option<u8>,
}

impl std::fmt::Display for GlobalStateObjectMetaItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {:?})", self.selector, self.access, self.depth)
    }
}

pub trait ObjectSelectorDataProvider: Send + Sync {
    fn object_id(&self) -> &ObjectId;
    fn obj_type(&self) -> u16;

    fn object_dec_id(&self) -> &Option<ObjectId>;
    fn object_author(&self) -> &Option<ObjectId>;
    fn object_owner(&self) -> &Option<ObjectId>;

    fn object_create_time(&self) -> Option<u64>;
    fn object_update_time(&self) -> Option<u64>;
    fn object_expired_time(&self) -> Option<u64>;

    fn update_time(&self) -> &u64;
    fn insert_time(&self) -> &u64;
    fn last_access_time(&self) -> &u64;
}

pub struct ObjectSelectorTokenList;

impl ObjectSelectorTokenList {
    fn gen_token_list() -> ExpReservedTokenList {
        let mut token_list = ExpReservedTokenList::new();

        // token_list.add_string("object_id");
        token_list.add_u16("obj_type_code");
        token_list.add_string("obj_category");
        token_list.add_u16("obj_type");

        token_list.add_string("object.dec_id");
        token_list.add_string("object.author");
        token_list.add_string("object.owner");

        token_list.add_u64("object.create_time");
        token_list.add_u64("object.update_time");
        token_list.add_u64("object.expired_time");

        token_list.add_u64("insert_time");
        token_list.add_u64("update_time");
        token_list.add_u64("last_access_time");

        token_list
    }

    pub fn token_list() -> &'static ExpReservedTokenList {
        static S_INSTANCE: OnceCell<ExpReservedTokenList> = OnceCell::new();
        S_INSTANCE.get_or_init(|| Self::gen_token_list())
    }
}

impl<T> ExpReservedTokenTranslator for T
where
    T: ObjectSelectorDataProvider,
{
    fn trans(&self, token: &str) -> ExpTokenEvalValue {
        match token {
            "obj_type_code" => ExpTokenEvalValue::U16(self.object_id().obj_type_code().into()),
            "obj_category" => ExpTokenEvalValue::from_string(&self.object_id().object_category()),
            "obj_type" => ExpTokenEvalValue::U16(self.obj_type()),

            "object.dec_id" => ExpTokenEvalValue::from_opt_string(&self.object_dec_id()),
            "object.author" => ExpTokenEvalValue::from_opt_string(&self.object_author()),
            "object.owner" => ExpTokenEvalValue::from_opt_string(&self.object_owner()),

            "object.create_time" => ExpTokenEvalValue::from_opt_u64(self.object_create_time()),
            "object.update_time" => ExpTokenEvalValue::from_opt_u64(self.object_update_time()),
            "object.expired_time" => ExpTokenEvalValue::from_opt_u64(self.object_expired_time()),

            "insert_time" => ExpTokenEvalValue::U64(*self.insert_time()),
            "update_time" => ExpTokenEvalValue::U64(*self.update_time()),
            "last_access_time" => ExpTokenEvalValue::U64(*self.last_access_time()),

            _ => {
                unreachable!("unknown object selector token! {}", token);
            }
        }
    }
}

