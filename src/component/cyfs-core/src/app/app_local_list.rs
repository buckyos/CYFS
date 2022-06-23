use crate::codec::*;
use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use cyfs_base::*;
use serde::Serialize;

use std::collections::HashSet;

// 一些内置的categroy
pub const APP_LOCAL_LIST_CATEGORY_APP: &str = "app";
pub const APP_LOCAL_LIST_PATH: &str = "/app/manager/local_list";

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppLocalListDesc)]
pub struct AppLocalListDesc {
    id: String,
    list: HashSet<DecAppId>,
}

impl DescContent for AppLocalListDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppLocalList as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl ProtobufTransform<protos::AppLocalListDesc> for AppLocalListDesc {
    fn transform(value: protos::AppLocalListDesc) -> BuckyResult<Self> {
        let mut list: HashSet<DecAppId> = HashSet::new();
        for item in value.list {
            let app_id = ProtobufCodecHelper::decode_buf(item.app_id)?;
            list.insert(app_id);
        }

        Ok(Self { id: value.id, list })
    }
}

impl ProtobufTransform<&AppLocalListDesc> for protos::AppLocalListDesc {
    fn transform(value: &AppLocalListDesc) -> BuckyResult<Self> {
        let mut ret = Self {
            id: value.id.to_owned(),
            list: vec![],
        };
        let mut list = Vec::new();
        for v in &value.list {
            let item = protos::AppLocalListItem {
                app_id: v.to_vec()?,
            };
            list.push(item);
        }
        ret.list = list.into();

        Ok(ret)
    }
}

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct AppLocalListBody {}

impl BodyContent for AppLocalListBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppLocalListType = NamedObjType<AppLocalListDesc, AppLocalListBody>;
type AppLocalListBuilder = NamedObjectBuilder<AppLocalListDesc, AppLocalListBody>;

pub type AppLocalListId = NamedObjectId<AppLocalListType>;
pub type AppLocalList = NamedObjectBase<AppLocalListType>;

pub trait AppLocalListObj {
    fn insert(&mut self, id: DecAppId);
    fn create(owner: ObjectId, id: &str) -> Self;
    fn remove(&mut self, id: &DecAppId);
    fn clear(&mut self);
    fn app_list(&self) -> &HashSet<DecAppId>;
    fn id(&self) -> &str;
    fn exists(&self, id: &DecAppId) -> bool;
}

impl AppLocalListObj for AppLocalList {
    fn create(owner: ObjectId, id: &str) -> Self {
        let body = AppLocalListBody {};
        let desc = AppLocalListDesc {
            id: id.to_owned(),
            list: HashSet::new(),
        };
        AppLocalListBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn insert(&mut self, id: DecAppId) {
        self.desc_mut().content_mut().list.insert(id);
    }

    fn remove(&mut self, id: &DecAppId) {
        self.desc_mut().content_mut().list.remove(id);
    }

    fn clear(&mut self) {
        self.desc_mut().content_mut().list.clear();
    }

    fn app_list(&self) -> &HashSet<DecAppId> {
        &self.desc().content().list
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn exists(&self, id: &DecAppId) -> bool {
        self.desc().content().list.contains(id)
    }
}
