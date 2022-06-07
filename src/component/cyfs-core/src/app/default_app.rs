use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use cyfs_base::*;
use std::collections::{hash_map, HashMap};
use serde::Serialize;

// 默认app的描述信息
#[derive(RawEncode, RawDecode, Clone, Debug, Serialize)]
pub struct DefaultAppInfo {
    pub name: String,
    pub desc: String,
    pub copyright: String,
    pub dec_id: DecAppId,
}

#[derive(RawEncode, RawDecode, Clone, Serialize)]
pub struct DefaultAppListDescContent {
    id: String,
}

impl DescContent for DefaultAppListDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::DefaultAppList as u16
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(RawEncode, RawDecode, Clone, Serialize)]
pub struct DefaultAppListContent {
    list: HashMap<String, DefaultAppInfo>,
}

impl BodyContent for DefaultAppListContent {}

type DefaultAppListType = NamedObjType<DefaultAppListDescContent, DefaultAppListContent>;
type DefaultAppListBuilder = NamedObjectBuilder<DefaultAppListDescContent, DefaultAppListContent>;
type DefaultAppListDesc = NamedObjectDesc<DefaultAppListDescContent>;

pub type DefaultAppListId = NamedObjectId<DefaultAppListType>;
pub type DefaultAppList = NamedObjectBase<DefaultAppListType>;

pub trait DefaultAppListObj {
    fn create(owner: ObjectId, id: &str) -> Self;
    fn id(&self) -> &str;

    fn set(&mut self, group: &str, app: DefaultAppInfo);
    fn remove(&mut self, group: &str, dec_id: Option<DecAppId>) -> Option<DefaultAppInfo>;
    fn app_list(&self) -> &HashMap<String, DefaultAppInfo>;
    fn get(&self, group: &str) -> Option<&DefaultAppInfo>;

    fn generate_id(owner: ObjectId, id: &str) -> ObjectId;
}

impl DefaultAppListObj for DefaultAppList {
    fn create(owner: ObjectId, id: &str) -> Self {
        let body = DefaultAppListContent {
            list: HashMap::new(),
        };
        let desc = DefaultAppListDescContent { id: id.to_owned() };
        DefaultAppListBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn set(&mut self, group: &str, info: DefaultAppInfo) {
        match self
            .body_mut_expect("")
            .content_mut()
            .list
            .entry(group.to_owned())
        {
            hash_map::Entry::Vacant(v) => {
                info!(
                    "will add new default app for group={}, app={:?}",
                    group, info
                );
                v.insert(info);
            }
            hash_map::Entry::Occupied(mut v) => {
                let c = v.get_mut();
                if c.dec_id != info.dec_id {
                    info!(
                        "will change default app for group={}, old={:?}, new app={:?}",
                        group, c, info
                    );
                    *c = info;
                } else {
                    info!(
                        "will update default app info for group={}, app={:?}",
                        group, info
                    );
                    *c = info;
                }
            }
        }

        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn remove(&mut self, group: &str, dec_id: Option<DecAppId>) -> Option<DefaultAppInfo> {
        let c = self.body_mut_expect("").content_mut().list.get(group);
        if c.is_none() {
            warn!(
                "revoke defualt app for group but not found! group={}",
                group
            );
            return None;
        }

        let c = c.unwrap();

        let ret = match dec_id {
            Some(id) => {
                if c.dec_id == id {
                    info!("will revoke default app for group={}, app={:?}", group, c);
                    drop(c);
                    self.body_mut_expect("").content_mut().list.remove(group)
                } else {
                    error!("revoke default app for group but dec_id not match! group={}, expect={}, got={}", group, id, c.dec_id);
                    None
                }
            }
            None => {
                info!("will revoke default app for group={}, app={:?}", group, c);
                drop(c);
                self.body_mut_expect("").content_mut().list.remove(group)
            }
        };

        if ret.is_some() {
            self.body_mut_expect("")
                .increase_update_time(bucky_time_now());
        }

        ret
    }

    fn get(&self, group: &str) -> Option<&DefaultAppInfo> {
        self.body_expect("").content().list.get(group)
    }

    fn app_list(&self) -> &HashMap<String, DefaultAppInfo> {
        &self.body_expect("").content().list
    }

    fn generate_id(owner: ObjectId, id: &str) -> ObjectId {
        Self::create(owner, id).desc().calculate_id()
    }
}
