use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use crate::codec::protos;
use serde::Serialize;

use std::collections::hash_map::RandomState;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::DecAppDescContent)]
pub struct DecAppDescContent {
    id: String,
}

impl DescContent for DecAppDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::DecApp as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::DecAppContent)]
pub struct DecAppContent {
    source: HashMap<String, ObjectId>,
    icon: Option<String>,
    desc: Option<String>,
    source_desc: HashMap<String, String>,
    tags: HashMap<String, String>
}

impl BodyContent for DecAppContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl ProtobufTransform<protos::DecAppContent> for DecAppContent {
    fn transform(value: protos::DecAppContent) -> BuckyResult<Self> {
        let mut source = HashMap::new();
        for item in value.source {
            source.insert(item.key, ObjectId::clone_from_slice(item.value.as_slice()));
        }

        let mut source_desc = HashMap::new();
        for item in value.source_desc {
            source_desc.insert(item.key, item.value);
        }

        let mut tags = HashMap::new();
        for item in value.tags {
            tags.insert(item.key, item.value);
        }

        let mut ret = DecAppContent {
            source,
            source_desc,
            icon: None,
            desc: None,
            tags
        };

        if value.icon.is_some() {
            ret.icon = Some(value.icon.unwrap());
        }
        if value.desc.is_some() {
            ret.desc = Some(value.desc.unwrap());
        }

        Ok(ret)
    }
}
impl ProtobufTransform<&DecAppContent> for protos::DecAppContent {
    fn transform(value: &DecAppContent) -> BuckyResult<Self> {
        let source_map: BTreeMap<String, ObjectId> = value.source.clone().into_iter().collect();
        let mut source = vec![];
        for (k, v) in source_map {
            source.push(protos::StringBytesMapItem{key: k, value: v.to_vec()?});
        }

        let source_desc_map: BTreeMap<String,String> = value.source_desc.clone().into_iter().collect();
        let mut source_desc = vec![];
        for (k, v) in source_desc_map {
            source_desc.push(protos::StringStringMapItem {key: k, value: v});
        }

        let tags_map: BTreeMap<String,String> = value.tags.clone().into_iter().collect();
        let mut tags = vec![];
        for (k, v) in tags_map {
            tags.push(protos::StringStringMapItem {key: k, value: v});
        }

        let mut ret = Self {
            source,
            source_desc,
            icon: None,
            desc: None,
            tags
        };

        if let Some(icon) = &value.icon {
            ret.icon = Some(icon.to_owned());
        }
        if let Some(desc) = &value.desc {
            ret.desc = Some(desc.to_owned());
        }

        Ok(ret)
    }
}

type DecAppType = NamedObjType<DecAppDescContent, DecAppContent>;
type DecAppBuilder = NamedObjectBuilder<DecAppDescContent, DecAppContent>;
type DecAppDesc = NamedObjectDesc<DecAppDescContent>;

pub type DecAppId = NamedObjectId<DecAppType>;
pub type DecApp = NamedObjectBase<DecAppType>;

pub trait DecAppObj {
    fn create(owner: ObjectId, id: &str) -> Self;
    fn name(&self) -> &str;
    fn app_desc(&self) -> Option<&str>;
    fn icon(&self) -> Option<&str>;
    fn find_source(&self, version: &str) -> BuckyResult<ObjectId>;
    fn find_source_desc(&self, version: &str) -> Option<&str>;
    fn remove_source(&mut self, version: &str);
    fn clear_source(&mut self);
    fn set_source(&mut self, version: String, id: ObjectId, desc: Option<String>);
    fn source(&self) -> &HashMap<String, ObjectId>;

    fn find_tag(&self, tag: &str) -> BuckyResult<&str>;
    fn set_tag(&mut self, tag: String, version: String);
    fn remove_tag(&mut self, tag: &str);
    fn tags(&self) -> &HashMap<String, String>;

    fn generate_id(owner: ObjectId, id: &str) -> ObjectId;
}

// 同owner, 同id的AppId应该始终相同，允许不同的话会造成混乱
impl DecAppObj for DecApp {
    fn create(owner: ObjectId, id: &str) -> Self {
        let body = DecAppContent {
            source: HashMap::new(),
            icon: None,
            desc: None,
            source_desc: HashMap::new(),
            tags: HashMap::new(),
        };
        let desc = DecAppDescContent { id: id.to_owned() };
        DecAppBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn name(&self) -> &str {
        &self.desc().content().id
    }

    fn app_desc(&self) -> Option<&str> {
        self.body_expect("").content().desc.as_deref()
    }

    fn icon(&self) -> Option<&str> {
        self.body_expect("").content().icon.as_deref()
    }

    fn find_source(&self, version: &str) -> BuckyResult<ObjectId> {
        self.body_expect("")
            .content()
            .source
            .get(version)
            .cloned()
            .ok_or(BuckyError::from(BuckyErrorCode::NotFound))
    }

    fn find_source_desc(&self, version: &str) -> Option<&str> {
        self.body_expect("")
            .content()
            .source_desc
            .get(version)
            .map(String::as_str)
    }

    fn remove_source(&mut self, version: &str) {
        self.body_mut_expect("")
            .content_mut()
            .source
            .remove(version);
        self.body_mut_expect("")
            .content_mut()
            .source_desc
            .remove(version);
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn clear_source(&mut self) {
        self.body_mut_expect("").content_mut().source.clear();
        self.body_mut_expect("").content_mut().source_desc.clear();
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn set_source(&mut self, version: String, id: ObjectId, desc: Option<String>) {
        self.body_mut_expect("")
            .content_mut()
            .source
            .insert(version.clone(), id);
        if let Some(desc) = desc {
            self.body_mut_expect("")
                .content_mut()
                .source_desc
                .insert(version, desc);
        }
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn source(&self) -> &HashMap<String, ObjectId, RandomState> {
        &self.body_expect("").content().source
    }

    fn find_tag(&self, tag: &str) -> BuckyResult<&str> {
        self.body_expect("")
            .content()
            .tags
            .get(tag)
            .map(String::as_str)
            .ok_or(BuckyError::from(BuckyErrorCode::NotFound))
    }

    fn set_tag(&mut self, tag: String, version: String) {
        self.body_mut_expect("")
            .content_mut()
            .tags
            .insert(tag, version);
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn remove_tag(&mut self, tag: &str) {
        self.body_mut_expect("")
            .content_mut()
            .tags
            .remove(tag);
    }

    fn tags(&self) -> &HashMap<String, String> {
        &self.body_expect("").content().tags
    }

    fn generate_id(owner: ObjectId, id: &str) -> ObjectId {
        Self::create(owner, id).desc().calculate_id()
    }
}