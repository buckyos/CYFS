use itertools::Itertools;

use crate::codec as cyfs_base;
use crate::protos::standard_objects;
use crate::*;

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::str::FromStr;

pub enum GroupMemberScope {
    Admin,
    Member,
    All,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum GroupDescContent {
    SimpleGroup(SimpleGroupDescContent),
    Org(OrgDescContent),
}

impl GroupDescContent {
    pub fn founder_id(&self) -> &Option<ObjectId> {
        match self {
            GroupDescContent::SimpleGroup(desc) => &desc.founder_id,
            GroupDescContent::Org(desc) => &desc.founder_id,
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum GroupBodyContent {
    SimpleGroup(SimpleGroupBodyContent),
    Org(OrgBodyContent),
}

impl DescContent for GroupDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Group.into()
    }

    type OwnerType = SubDescNone;
    type AreaType = Option<Area>;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl BodyContent for GroupBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_RAW
    }
}

impl GroupBodyContent {
    pub fn name(&self) -> &Option<String> {
        &self.common().name
    }

    pub fn icon(&self) -> &Option<String> {
        &self.common().icon
    }

    pub fn description(&self) -> &Option<String> {
        &self.common().description
    }

    pub fn members(&self) -> &HashMap<ObjectId, GroupMember> {
        &self.common().members
    }

    pub fn members_mut(&mut self) -> &mut HashMap<ObjectId, GroupMember> {
        &mut self.common_mut().members
    }

    pub fn ood_list(&self) -> &Vec<DeviceId> {
        &self.common().ood_list
    }

    pub fn ood_list_mut(&mut self) -> &mut Vec<DeviceId> {
        &mut self.common_mut().ood_list
    }

    pub fn version(&self) -> u64 {
        self.common().version
    }

    pub fn prev_shell_id(&self) -> &Option<ObjectId> {
        &self.common().prev_shell_id
    }

    fn common(&self) -> &CommonGroupBodyContent {
        match self {
            GroupBodyContent::Org(body) => &body.common,
            GroupBodyContent::SimpleGroup(body) => &body.common,
        }
    }

    fn common_mut(&mut self) -> &mut CommonGroupBodyContent {
        match self {
            GroupBodyContent::Org(body) => &mut body.common,
            GroupBodyContent::SimpleGroup(body) => &mut body.common,
        }
    }
}

pub type GroupType = NamedObjType<GroupDescContent, GroupBodyContent>;
pub type GroupBuilder = NamedObjectBuilder<GroupDescContent, GroupBodyContent>;

pub type GroupDesc = NamedObjectDesc<GroupDescContent>;
pub type GroupId = NamedObjectId<GroupType>;
pub type Group = NamedObjectBase<GroupType>;

impl GroupDesc {
    pub fn group_id(&self) -> GroupId {
        GroupId::try_from(self.calculate_id()).unwrap()
    }
}

impl Group {
    pub fn new_simple_group(
        founder_id: Option<ObjectId>,
        admins: Vec<GroupMember>,
        area: Area,
    ) -> GroupBuilder {
        let desc_content = SimpleGroupDescContent {
            unique_id: UniqueId::create_with_random(),
            admins: HashMap::from_iter(admins.into_iter().map(|m| (m.id, m))),
            founder_id,
        };

        let body_content = SimpleGroupBodyContent::default();

        GroupBuilder::new(
            GroupDescContent::SimpleGroup(desc_content),
            GroupBodyContent::SimpleGroup(body_content),
        )
        .area(area)
    }

    pub fn new_org(founder_id: Option<ObjectId>, area: Area) -> GroupBuilder {
        let desc_content = OrgDescContent {
            founder_id,
            unique_id: UniqueId::create_with_random(),
        };

        let body_content = OrgBodyContent::default();

        GroupBuilder::new(
            GroupDescContent::Org(desc_content),
            GroupBodyContent::Org(body_content),
        )
        .area(area)
    }

    pub fn founder_id(&self) -> &Option<ObjectId> {
        match self.desc().content() {
            GroupDescContent::SimpleGroup(s) => &s.founder_id,
            GroupDescContent::Org(o) => &o.founder_id,
        }
    }

    pub fn name(&self) -> &Option<String> {
        &self.common().name
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.common_mut().name = name;
    }

    pub fn icon(&self) -> &Option<String> {
        &self.common().icon
    }

    pub fn set_icon(&mut self, icon: Option<String>) {
        self.common_mut().icon = icon;
    }

    pub fn description(&self) -> &Option<String> {
        &self.common().description
    }

    pub fn set_description(&mut self, description: Option<String>) {
        self.common_mut().description = description;
    }

    pub fn admins(&self) -> &HashMap<ObjectId, GroupMember> {
        if self.is_org() {
            &self.check_org_body_content().admins
        } else {
            &self.check_simple_group_desc_content().admins
        }
    }

    pub fn members(&self) -> &HashMap<ObjectId, GroupMember> {
        &self.common().members
    }

    pub fn set_members(&mut self, members: Vec<GroupMember>) {
        self.common_mut().members = HashMap::from_iter(members.into_iter().map(|m| (m.id, m)));
    }

    pub fn ood_list(&self) -> &Vec<DeviceId> {
        &self.common().ood_list
    }

    pub fn set_ood_list(&mut self, oods: Vec<DeviceId>) {
        self.common_mut().ood_list = HashSet::<DeviceId>::from_iter(oods.into_iter())
            .into_iter()
            .sorted()
            .collect();
    }

    pub fn contain_ood(&self, ood_id: &ObjectId) -> bool {
        match DeviceId::try_from(ood_id) {
            Ok(device_id) => self.ood_list().contains(&device_id),
            Err(_) => false,
        }
    }

    pub fn is_same_ood_list(&self, other: &Group) -> bool {
        let my_oods = self.ood_list();
        let other_oods = other.ood_list();

        if my_oods.len() != other_oods.len() {
            return false;
        }

        for id in my_oods {
            if !other_oods.contains(id) {
                return false;
            }
        }

        true
    }

    pub fn version(&self) -> u64 {
        self.common().version
    }

    pub fn set_version(&mut self, version: u64) {
        self.common_mut().version = version;
    }

    pub fn prev_shell_id(&self) -> &Option<ObjectId> {
        &self.common().prev_shell_id
    }

    pub fn set_prev_shell_id(&mut self, prev_shell_id: Option<ObjectId>) {
        self.common_mut().prev_shell_id = prev_shell_id;
    }

    pub fn is_simple_group(&self) -> bool {
        match self.desc().content() {
            GroupDescContent::SimpleGroup(_) => true,
            _ => false,
        }
    }

    pub fn is_org(&self) -> bool {
        match self.desc().content() {
            GroupDescContent::Org(_) => true,
            _ => false,
        }
    }

    pub fn check_simple_group_desc_content(&self) -> &SimpleGroupDescContent {
        match self.desc().content() {
            GroupDescContent::SimpleGroup(desc) => desc,
            _ => panic!("group type not match, expect: simple"),
        }
    }

    pub fn check_org_desc_content(&self) -> &OrgDescContent {
        match self.desc().content() {
            GroupDescContent::Org(desc) => desc,
            _ => panic!("group type not match, expect: org"),
        }
    }

    pub fn check_simple_group_body_content(&self) -> &SimpleGroupBodyContent {
        match self.body().as_ref().unwrap().content() {
            GroupBodyContent::SimpleGroup(body) => body,
            _ => panic!("group type not match, expect: simple"),
        }
    }

    pub fn check_org_body_content(&self) -> &OrgBodyContent {
        match self.body().as_ref().unwrap().content() {
            GroupBodyContent::Org(body) => body,
            _ => panic!("group type not match, expect: org"),
        }
    }

    pub fn check_simple_group_body_content_mut(&mut self) -> &mut SimpleGroupBodyContent {
        match self.body_mut().as_mut().unwrap().content_mut() {
            GroupBodyContent::SimpleGroup(body) => body,
            _ => panic!("group type not match, expect: simple"),
        }
    }

    pub fn check_org_body_content_mut(&mut self) -> &mut OrgBodyContent {
        match self.body_mut().as_mut().unwrap().content_mut() {
            GroupBodyContent::Org(body) => body,
            _ => panic!("group type not match, expect: org"),
        }
    }

    pub fn select_members_with_distance(
        &self,
        target: &ObjectId,
        scope: GroupMemberScope,
    ) -> Vec<&ObjectId> {
        let mut members = match scope {
            GroupMemberScope::Admin => self.admins().keys().collect::<Vec<_>>(),
            GroupMemberScope::Member => self.members().keys().collect::<Vec<_>>(),
            GroupMemberScope::All => [
                self.admins().keys().collect::<Vec<_>>(),
                self.members().keys().collect::<Vec<_>>(),
            ]
            .concat(),
        };

        members.sort_unstable_by(|l, r| {
            let dl = l.distance_of(target);
            let dr = r.distance_of(target);
            dl.cmp(&dr)
        });
        members
    }

    pub fn ood_list_with_distance(&self, target: &ObjectId) -> Vec<&ObjectId> {
        let oods = self
            .ood_list()
            .iter()
            .map(|id| id.object_id())
            .sorted_unstable_by(|l, r| {
                let dl = l.distance_of(target);
                let dr = r.distance_of(target);
                dl.cmp(&dr)
            })
            .collect::<Vec<_>>();
        oods
    }

    fn common(&self) -> &CommonGroupBodyContent {
        self.body().as_ref().unwrap().content().common()
    }

    fn common_mut(&mut self) -> &mut CommonGroupBodyContent {
        self.body_mut().as_mut().unwrap().content_mut().common_mut()
    }
}

#[derive(Clone, Debug)]
pub struct GroupMember {
    pub id: ObjectId,
    pub title: String,
}

impl GroupMember {
    pub fn new(id: ObjectId, title: String) -> Self {
        GroupMember { id, title }
    }
    pub fn from_member_id(id: ObjectId) -> GroupMember {
        GroupMember {
            id,
            title: "".to_string(),
        }
    }
}

impl TryFrom<protos::GroupMember> for GroupMember {
    type Error = BuckyError;

    fn try_from(value: protos::GroupMember) -> BuckyResult<Self> {
        let ret = Self {
            id: ProtobufCodecHelper::decode_buf(value.id)?,
            title: value.title,
        };

        Ok(ret)
    }
}

impl TryFrom<&GroupMember> for protos::GroupMember {
    type Error = BuckyError;

    fn try_from(value: &GroupMember) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.id = value.id.to_vec()?;
        ret.title = value.title.clone();

        Ok(ret)
    }
}

impl FromStr for GroupMember {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut fields = s.split(":");

        let id = if let Some(id) = fields.next() {
            PeopleId::from_str(id)?
        } else {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidFormat,
                "need peopleid of member.",
            ));
        };

        let title = fields.next().unwrap_or("");

        Ok(Self {
            id: id.object_id().clone(),
            title: title.to_string(),
        })
    }
}

impl ToString for &GroupMember {
    fn to_string(&self) -> String {
        format!("{}:{}", self.id, self.title)
    }
}

#[derive(Clone, Debug, Default)]
struct CommonGroupBodyContent {
    name: Option<String>,
    icon: Option<String>,
    description: Option<String>,

    members: HashMap<ObjectId, GroupMember>,

    ood_list: Vec<DeviceId>,

    version: u64,
    prev_shell_id: Option<ObjectId>,
}

impl CommonGroupBodyContent {
    fn new(
        name: Option<String>,
        icon: Option<String>,
        description: Option<String>,
        members: Vec<GroupMember>,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            name,
            icon,
            description,
            members: HashMap::from_iter(members.into_iter().map(|m| (m.id, m))),
            ood_list: HashSet::<DeviceId>::from_iter(ood_list.into_iter())
                .into_iter()
                .sorted()
                .collect::<Vec<_>>(),
            version: 0,
            prev_shell_id: None,
        }
    }
}

impl TryFrom<protos::CommonGroupBodyContent> for CommonGroupBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::CommonGroupBodyContent) -> BuckyResult<Self> {
        let mut ood_list = ProtobufCodecHelper::decode_buf_list(value.take_ood_list())?;
        ood_list.sort();

        let ret = Self {
            name: if value.has_name() {
                Some(value.take_name())
            } else {
                None
            },
            icon: if value.has_icon() {
                Some(value.take_icon())
            } else {
                None
            },
            description: if value.has_description() {
                Some(value.take_description())
            } else {
                None
            },
            members:
                HashMap::from_iter(
                    ProtobufCodecHelper::decode_value_list::<
                        GroupMember,
                        standard_objects::GroupMember,
                    >(value.take_members())?
                    .into_iter()
                    .map(|m| (m.id, m)),
                ),
            ood_list,
            version: value.version,
            prev_shell_id: if value.has_prev_shell_id() {
                Some(ProtobufCodecHelper::decode_buf(value.take_prev_shell_id())?)
            } else {
                None
            },
        };

        Ok(ret)
    }
}

impl TryFrom<&CommonGroupBodyContent> for protos::CommonGroupBodyContent {
    type Error = BuckyError;

    fn try_from(value: &CommonGroupBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        if let Some(name) = value.name.as_ref() {
            ret.set_name(name.clone());
        }
        if let Some(icon) = value.icon.as_ref() {
            ret.set_icon(icon.clone());
        }
        if let Some(description) = value.description.as_ref() {
            ret.set_description(description.clone());
        }

        let members = value
            .members
            .values()
            .sorted_by(|l, r| l.id.cmp(&r.id))
            .map(|m| m.clone())
            .collect::<Vec<_>>();
        ret.set_members(ProtobufCodecHelper::encode_nested_list(&members)?);

        let oods = value
            .ood_list
            .iter()
            .sorted()
            .map(|id| id.clone())
            .collect::<Vec<_>>();
        ret.set_ood_list(ProtobufCodecHelper::encode_buf_list(oods.as_slice())?);

        ret.version = value.version;
        if let Some(prev_shell_id) = &value.prev_shell_id {
            ret.set_prev_shell_id(prev_shell_id.to_vec()?);
        }

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct SimpleGroupDescContent {
    unique_id: UniqueId,
    founder_id: Option<ObjectId>,
    admins: HashMap<ObjectId, GroupMember>,
}

impl SimpleGroupDescContent {
    pub fn admins(&self) -> &HashMap<ObjectId, GroupMember> {
        &self.admins
    }
}

impl TryFrom<protos::SimpleGroupDescContent> for SimpleGroupDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::SimpleGroupDescContent) -> BuckyResult<Self> {
        let ret = Self {
            founder_id: if value.has_founder_id() {
                ProtobufCodecHelper::decode_buf(value.take_founder_id())?
            } else {
                None
            },
            unique_id: ProtobufCodecHelper::decode_buf(value.unique_id)?,
            admins:
                HashMap::from_iter(
                    ProtobufCodecHelper::decode_value_list::<
                        GroupMember,
                        standard_objects::GroupMember,
                    >(value.admins)?
                    .into_iter()
                    .map(|m| (m.id, m)),
                ),
        };

        Ok(ret)
    }
}

impl TryFrom<&SimpleGroupDescContent> for protos::SimpleGroupDescContent {
    type Error = BuckyError;

    fn try_from(value: &SimpleGroupDescContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.unique_id = value.unique_id.to_vec()?;
        if let Some(founder_id) = value.founder_id.as_ref() {
            ret.set_founder_id(founder_id.to_vec()?);
        }

        let admins = value
            .admins
            .values()
            .sorted_by(|l, r| l.id.cmp(&r.id))
            .map(|m| m.clone())
            .collect::<Vec<_>>();
        ret.set_admins(ProtobufCodecHelper::encode_nested_list(&admins)?);

        Ok(ret)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimpleGroupBodyContent {
    common: CommonGroupBodyContent,
}

impl SimpleGroupBodyContent {
    fn new(
        name: Option<String>,
        icon: Option<String>,
        description: Option<String>,
        members: Vec<GroupMember>,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            common: CommonGroupBodyContent::new(name, icon, description, members, ood_list),
        }
    }
}

impl TryFrom<protos::SimpleGroupBodyContent> for SimpleGroupBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::SimpleGroupBodyContent) -> BuckyResult<Self> {
        let ret = Self {
            common: ProtobufCodecHelper::decode_value(value.take_common())?,
        };

        Ok(ret)
    }
}

impl TryFrom<&SimpleGroupBodyContent> for protos::SimpleGroupBodyContent {
    type Error = BuckyError;

    fn try_from(value: &SimpleGroupBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_common(ProtobufCodecHelper::encode_nested_item(&value.common)?);

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct OrgDescContent {
    unique_id: UniqueId,
    founder_id: Option<ObjectId>,
}

impl TryFrom<protos::OrgDescContent> for OrgDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::OrgDescContent) -> BuckyResult<Self> {
        let ret = Self {
            founder_id: if value.has_founder_id() {
                Some(ProtobufCodecHelper::decode_buf(value.take_founder_id())?)
            } else {
                None
            },
            unique_id: ProtobufCodecHelper::decode_buf(value.unique_id)?,
        };

        Ok(ret)
    }
}

impl TryFrom<&OrgDescContent> for protos::OrgDescContent {
    type Error = BuckyError;

    fn try_from(value: &OrgDescContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.unique_id = value.unique_id.to_vec()?;
        if let Some(founder_id) = value.founder_id.as_ref() {
            ret.set_founder_id(founder_id.to_vec()?);
        }

        Ok(ret)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OrgBodyContent {
    admins: HashMap<ObjectId, GroupMember>,
    common: CommonGroupBodyContent,
}

impl OrgBodyContent {
    fn new(
        name: Option<String>,
        icon: Option<String>,
        description: Option<String>,
        admins: Vec<GroupMember>,
        members: Vec<GroupMember>,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            common: CommonGroupBodyContent::new(name, icon, description, members, ood_list),
            admins: HashMap::from_iter(admins.into_iter().map(|m| (m.id, m))),
        }
    }

    pub fn admins(&self) -> &HashMap<ObjectId, GroupMember> {
        &self.admins
    }

    pub fn set_admins(&mut self, admins: Vec<GroupMember>) {
        self.admins = HashMap::from_iter(admins.into_iter().map(|m| (m.id, m)));
    }
}

impl TryFrom<protos::OrgBodyContent> for OrgBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::OrgBodyContent) -> BuckyResult<Self> {
        let ret = Self {
            admins:
                HashMap::from_iter(
                    ProtobufCodecHelper::decode_value_list::<
                        GroupMember,
                        standard_objects::GroupMember,
                    >(value.take_admins())?
                    .into_iter()
                    .map(|m| (m.id, m)),
                ),
            common: ProtobufCodecHelper::decode_value(value.take_common())?,
        };

        Ok(ret)
    }
}

impl TryFrom<&OrgBodyContent> for protos::OrgBodyContent {
    type Error = BuckyError;

    fn try_from(value: &OrgBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        let admins = value
            .admins
            .values()
            .sorted_by(|l, r| l.id.cmp(&r.id))
            .map(|m| m.clone())
            .collect::<Vec<_>>();

        ret.set_admins(ProtobufCodecHelper::encode_nested_list(&admins)?);
        ret.set_common(ProtobufCodecHelper::encode_nested_item(&value.common)?);

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(SimpleGroupDescContent);
crate::inner_impl_default_protobuf_raw_codec!(SimpleGroupBodyContent);

crate::inner_impl_default_protobuf_raw_codec!(OrgDescContent);
crate::inner_impl_default_protobuf_raw_codec!(OrgBodyContent);

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn simple_group() {
        // let threshold = 0;

        // let members = vec![ObjectId::default()];

        // let ood_list = vec![DeviceId::default()];

        // let obj = SimpleGroup::new(
        //     threshold,
        //     vec![],
        //     members,
        //     OODWorkMode::Standalone,
        //     ood_list,
        //     Area::default(),
        // )
        // .build();
        // // let p = Path::new("f:\\temp\\simple_group.obj");
        // // if p.parent().unwrap().exists() {
        // //     obj.clone().encode_to_file(p, false);
        // // }

        // let buf = obj.to_vec().unwrap();

        // let decode_obj = SimpleGroup::clone_from_slice(&buf).unwrap();

        // assert!(obj.desc().simple_group_id() == decode_obj.desc().simple_group_id());
    }
}
