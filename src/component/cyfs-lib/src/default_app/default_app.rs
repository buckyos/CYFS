use cyfs_base::*;
use cyfs_core::{DecApp, DecAppId, DecAppObj};

// 目前支持的默认app分组
pub const DEFAULT_APP_GROUP_IM: &str = "im";
pub const DEFAULT_APP_GROUP_MAIL: &str = "mail";

pub const DEFAULT_APP_GROUP_LIST: [&str; 2] = [DEFAULT_APP_GROUP_IM, DEFAULT_APP_GROUP_MAIL];

pub struct DefaultAppGroupManager;

impl DefaultAppGroupManager {
    pub fn is_known_group(group: &str) -> bool {
        DEFAULT_APP_GROUP_LIST
            .iter()
            .find(|v| **v == group)
            .is_some()
    }
}

// 可以直接使用group name，也可以使用默认的group dec_id来代表相应的group
pub enum DefaultAppGroup {
    DecAppId(DecAppId),
    Group(String),
}

struct DefaultApp {
    name: String,
    dec_id: ObjectId,
}

pub struct DefaultApps {
    owner: ObjectId,
    list: Vec<DefaultApp>,
}

pub const DEFAULT_APP_LIST: [&str;1] = ["im"];

impl DefaultApps {
    pub fn new() -> Self {
        let owner = PeopleId::default();
        let mut list = vec![];

        for name in &DEFAULT_APP_LIST {
            let id = DecApp::generate_id(owner.object_id().to_owned(), name);
            list.push(DefaultApp {
                name: (*name).to_owned(),
                dec_id: id,
            });    
        }

        Self {
            owner: owner.object_id().to_owned(),
            list,
        }
    }
    
    pub fn new_default_app(owner: &ObjectId, name: &str) -> ObjectId {
        DecApp::generate_id(owner.clone(), name)
    }


    pub fn get_default_app(&self, name: &str) -> Option<ObjectId> {
        self.list.iter().find(|v| {
            if v.name == name {
                true
            } else {
                false
            }
        }).map(|ret| {
            ret.dec_id.clone()
        })
    }
    
    pub fn get_default_app_group(&self, dec_id: &ObjectId) -> Option<String> {
        self.list.iter().find(|v| {
            if v.dec_id == *dec_id {
                true
            } else {
                false
            }
        }).map(|ret| {
            ret.name.clone()
        })
    }
}

lazy_static::lazy_static! {
    pub static ref DEFAULT_APPS: DefaultApps = DefaultApps::new();
}