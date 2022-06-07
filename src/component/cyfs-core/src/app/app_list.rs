use crate::app::app_status::{AppStatus};
use crate::coreobj::CoreObjectType;
use crate::{AppStatusObj, DecAppId};
use cyfs_base::*;
use crate::codec::*;

use serde::Serialize;

use std::collections::HashMap;

// 一些内置的categroy
pub const APPLIST_APP_CATEGORY: &str = "app";
pub const APPLIST_SERVICE_CATEGORY: &str = "service";

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppListDescContent)]
pub struct AppListDescContent {
    id: String,
    category: String,
}

impl DescContent for AppListDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::AppList as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::AppListContent)]
pub struct AppListContent {
    pub(crate) source: HashMap<DecAppId, AppStatus>,
}

impl BodyContent for AppListContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ProtobufTransform<protos::AppListContent> for AppListContent {
    fn transform(value: protos::AppListContent) -> BuckyResult<Self> {
        let mut source: HashMap<DecAppId, AppStatus> = HashMap::new();
        for item in value.source {
            let status = AppStatus::clone_from_slice(item.app_status.as_slice())?;
            let app_id = DecAppId::clone_from_slice(item.app_id.as_slice())?;
            if let Some(old) = source.insert(app_id, status) {
                error!("decode AppListContent source but got repeated item: {}", old.desc().calculate_id());
            }
        }

        Ok(Self {
            source
        })
    }
}
impl ProtobufTransform<&AppListContent> for protos::AppListContent {
    fn transform(value: &AppListContent) -> BuckyResult<Self> {
        let mut ret = Self { source: vec![] };
        let mut list = Vec::new();
        for (key, value) in &value.source {
            let mut item = protos::AppListSourceItem { app_id: vec![], app_status: vec![] };
            item.app_id = key.to_vec()?;
            item.app_status = value.to_vec()?;
            list.push(item);
        }
        ret.source = list;

        Ok(ret)
    }
}

type AppListType = NamedObjType<AppListDescContent, AppListContent>;
type AppListBuilder = NamedObjectBuilder<AppListDescContent, AppListContent>;
type AppListDesc = NamedObjectDesc<AppListDescContent>;

pub type AppListId = NamedObjectId<AppListType>;
pub type AppList = NamedObjectBase<AppListType>;

pub trait AppListObj {
    fn create(owner: ObjectId, id: &str, category: &str) -> Self;
    fn put(&mut self, app: AppStatus);
    fn remove(&mut self, id: &DecAppId);
    fn clear(&mut self);
    fn app_list(&self) -> &HashMap<DecAppId, AppStatus>;
    fn id(&self) -> &str;
    fn category(&self) -> &str;
    fn exists(&self, id: &DecAppId) -> bool;

    fn generate_id(owner: ObjectId, id: &str, category: &str) -> ObjectId;
}

impl AppListObj for AppList {
    fn create(owner: ObjectId, id: &str, category: &str) -> Self {
        let body = AppListContent {
            source: HashMap::new(),
        };
        let desc = AppListDescContent {
            id: id.to_owned(),
            category: category.to_owned(),
        };
        AppListBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn put(&mut self, app: AppStatus) {
        let id = app.app_id().clone();
        self.body_mut_expect("")
            .content_mut()
            .source
            .insert(id, app);
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn remove(&mut self, id: &DecAppId) {
        self.body_mut_expect("").content_mut().source.remove(id);
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn clear(&mut self) {
        self.body_mut_expect("").content_mut().source.clear();
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn app_list(&self) -> &HashMap<DecAppId, AppStatus> {
        &self.body_expect("").content().source
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn category(&self) -> &str {
        &self.desc().content().category
    }

    fn exists(&self, id: &DecAppId) -> bool {
        self.body_expect("").content().source.contains_key(id)
    }

    fn generate_id(owner: ObjectId, id: &str, category: &str) -> ObjectId {
        Self::create(owner, id, category).desc().calculate_id()
    }
}
