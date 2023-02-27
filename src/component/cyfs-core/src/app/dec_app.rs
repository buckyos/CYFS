use crate::codec::protos;
use crate::coreobj::CoreObjectType;
use cyfs_base::*;
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
    tags: HashMap<String, String>,
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
            source.insert(item.key, ObjectId::clone_from_slice(item.value.as_slice())?);
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
            tags,
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
            source.push(protos::StringBytesMapItem {
                key: k,
                value: v.to_vec()?,
            });
        }

        let source_desc_map: BTreeMap<String, String> =
            value.source_desc.clone().into_iter().collect();
        let mut source_desc = vec![];
        for (k, v) in source_desc_map {
            source_desc.push(protos::StringStringMapItem { key: k, value: v });
        }

        let tags_map: BTreeMap<String, String> = value.tags.clone().into_iter().collect();
        let mut tags = vec![];
        for (k, v) in tags_map {
            tags.push(protos::StringStringMapItem { key: k, value: v });
        }

        let mut ret = Self {
            source,
            source_desc,
            icon: None,
            desc: None,
            tags,
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

    // return (origin version, semversion);
    fn find_version(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<(&str, String)>;

    fn find_source_by_semver(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<ObjectId>;
    fn find_source(&self, version: &str) -> BuckyResult<ObjectId>;

    fn find_source_desc_by_semver(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<Option<&str>>;
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

pub struct SemVerHelper {}

impl SemVerHelper {
    // x.y.?.z => x.y.z
    // x.y.?.z-? => x.y.z-?
    pub fn fix_semver(ver: &str) -> String {
        let mut top: Vec<&str> = ver.split('-').collect();
        let mut ret: Vec<&str> = top[0].split('.').collect();
        if ret.len() == 4 {
            ret.remove(2);
        }
    
        let ret = ret.join(".");
        let ret = if top.len() > 1 {
            top[0] = &ret;
            top.join("-")
        } else {
            ret
        };

        // println!("{} -> {}", ver, ret);
        ret
    }
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

    // https://nodesource.com/blog/semver-tilde-and-caret/
    // When pre is specified, all matching prerelease versions will be included; 
    // otherwise, only all versions that do not contain any prerelease will be matched
    fn find_version(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<(&str, String)> {
        let name = self.name();
        let id = self.desc().calculate_id();

        let req_version = semver::VersionReq::parse(req_semver).map_err(|e| {
            let msg = format!(
                "invalid semver request string! id={}, name={}, value={}, pre={:?}, {}",
                id, name, req_semver, pre, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let list: Vec<_> = self
            .body_expect("")
            .content()
            .source
            .keys()
            .map(|key| {
                let new_version = SemVerHelper::fix_semver(&key);
                (key, new_version)
            })
            .collect();

        let mut semver_list = vec![];
        for (version, new_version) in list {
            let mut semver = semver::Version::parse(&new_version).map_err(|e| {
                let msg = format!(
                    "invalid semver string! id={}, name={}, value={}, pre={:?}, {}",
                    id, name, version, pre, e,
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            if !semver.pre.is_empty() {
                if let Some(pre) = pre {
                    if semver.pre.as_str() != pre {
                        continue;
                    }
                    semver.pre = semver::Prerelease::EMPTY;
                } else {
                    continue;
                }
            }

            semver_list.push((version, semver));
        }

        semver_list.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap());

        let ret = semver_list.iter().find(|(version, semver)| {
            if req_version.matches(semver) {
                info!(
                    "app version matched: id={}, name={}, req={}, got={}, prev={:?}",
                    id, name, req_semver, version, pre,
                );
                true
            } else {
                false
            }
        });

        if ret.is_none() {
            let msg = format!(
                "no matching semver found for app: id={}, name={}, req={}",
                id, name, req_semver
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (version, semver) = ret.unwrap();
        Ok((version, semver.to_string()))
    }

    fn find_source_by_semver(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<ObjectId> {
        let ret = self.find_version(req_semver, pre)?;
        self.find_source(&ret.0)
    }

    fn find_source(&self, version: &str) -> BuckyResult<ObjectId> {
        self.body_expect("")
            .content()
            .source
            .get(version)
            .cloned()
            .ok_or(BuckyError::from(BuckyErrorCode::NotFound))
    }

    fn find_source_desc_by_semver(&self, req_semver: &str, pre: Option<&str>) -> BuckyResult<Option<&str>> {
        let ret = self.find_version(req_semver, pre)?;
        Ok(self.find_source_desc(&ret.0))
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
        self.body_mut_expect("").content_mut().tags.remove(tag);
    }

    fn tags(&self) -> &HashMap<String, String> {
        &self.body_expect("").content().tags
    }

    fn generate_id(owner: ObjectId, id: &str) -> ObjectId {
        Self::create(owner, id).desc().calculate_id()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let owner = ObjectId::default();
        let mut dec_app = DecApp::create(owner.clone(), "test-dec-app");
        dec_app.set_source("1.0.0".to_owned(), owner.clone(), None);
        dec_app.set_source("1.0.1".to_owned(), owner.clone(), None);
        dec_app.set_source("1.0.2".to_owned(), owner.clone(), None);
        dec_app.set_source("1.1.2".to_owned(), owner.clone(), None);
        dec_app.set_source("1.1.5".to_owned(), owner.clone(), None);
        
        dec_app.set_source("1.3.7".to_owned(), owner.clone(), None);
        dec_app.set_source("1.3.10".to_owned(), owner.clone(), None);
        dec_app.set_source("1.4.0.20".to_owned(), owner.clone(), None);
        dec_app.set_source("1.4.1.21-preview".to_owned(), owner.clone(), None);
        dec_app.set_source("1.5.1.22-preview".to_owned(), owner.clone(), None);

        dec_app.set_source("2.5.28".to_owned(), owner.clone(), None);
        dec_app.set_source("2.5.30".to_owned(), owner.clone(), None);

        let ret = dec_app.find_version("*", None).unwrap();
        assert_eq!(ret, "2.5.30");

        let ret = dec_app.find_version("2.5.28", None).unwrap();
        assert_eq!(ret, "2.5.30");

        let ret = dec_app.find_version("=1.0", None).unwrap();
        assert_eq!(ret, "1.0.2");

        // ^ first none zero version seg, and is default if not present

        dec_app.find_version("=1.4.21-preview", None).unwrap_err();

        let ret = dec_app.find_version("=1.4.21", Some("preview")).unwrap();
        assert_eq!(ret, "1.4.1.21-preview");
        let ret = dec_app.find_version("1.4", Some("preview")).unwrap();
        assert_eq!(ret, "1.5.1.22-preview");
        let ret = dec_app.find_version("1.0", Some("preview")).unwrap();
        assert_eq!(ret, "1.5.1.22-preview");

        let ret = dec_app.find_version("~1.4", None).unwrap();
        assert_eq!(ret, "1.4.0.20");

        // ~ second none zero version seg
        let ret = dec_app.find_version("~1.1", None).unwrap();
        assert_eq!(ret, "1.1.5");

        let ret = dec_app.find_version("<1.3", None).unwrap();
        assert_eq!(ret, "1.1.5");

        let ret = dec_app.find_version("=1.3", None).unwrap();
        assert_eq!(ret, "1.3.10");

        let ret = dec_app.find_version("<=1.3.8", None).unwrap();
        assert_eq!(ret, "1.3.7");
    }
}