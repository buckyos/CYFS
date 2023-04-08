use crate::codec as cyfs_base;
use crate::*;

use std::convert::TryFrom;

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
    pub fn name(&self) -> &str {
        self.common().name.as_str()
    }

    pub fn icon(&self) -> &Option<FileId> {
        &self.common().icon
    }

    pub fn description(&self) -> &str {
        self.common().description.as_str()
    }

    pub fn members(&self) -> &Vec<GroupMember> {
        &self.common().members
    }

    pub fn members_mut(&mut self) -> &mut Vec<GroupMember> {
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

    pub fn prev_blob_id(&self) -> &Option<ObjectId> {
        &self.common().prev_blob_id
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
        founder_id: ObjectId,
        admins: Vec<GroupMember>,
        area: Area,
    ) -> GroupBuilder {
        let desc_content = SimpleGroupDescContent {
            unique_id: UniqueId::create_with_random(),
            admins,
            founder_id,
        };

        let body_content = SimpleGroupBodyContent::default();

        GroupBuilder::new(
            GroupDescContent::SimpleGroup(desc_content),
            GroupBodyContent::SimpleGroup(body_content),
        )
        .area(area)
    }

    pub fn new_org(founder_id: ObjectId, area: Area) -> GroupBuilder {
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

    pub fn name(&self) -> &str {
        self.common().name.as_str()
    }

    pub fn set_name(&mut self, name: String) {
        self.common_mut().name = name;
    }

    pub fn icon(&self) -> &Option<FileId> {
        &self.common().icon
    }

    pub fn set_icon(&mut self, icon: Option<FileId>) {
        self.common_mut().icon = icon;
    }

    pub fn description(&self) -> &str {
        self.common().description.as_str()
    }

    pub fn set_description(&mut self, description: String) {
        self.common_mut().description = description;
    }

    pub fn admins(&self) -> &[GroupMember] {
        if self.is_org() {
            self.check_org_body_content().admins.as_slice()
        } else {
            self.check_simple_group_desc_content().admins.as_slice()
        }
    }

    pub fn members(&self) -> &[GroupMember] {
        self.common().members.as_slice()
    }

    pub fn set_members(&mut self, members: Vec<GroupMember>) {
        self.common_mut().members = members;
    }

    pub fn ood_list(&self) -> &[DeviceId] {
        self.common().ood_list.as_slice()
    }

    pub fn set_ood_list(&mut self, oods: Vec<DeviceId>) {
        self.common_mut().ood_list = oods;
    }

    pub fn contain_ood(&self, ood_id: &ObjectId) -> bool {
        self.ood_list()
            .iter()
            .find(|id| id.object_id() == ood_id)
            .is_some()
    }

    pub fn is_same_ood_list(&self, other: &Group) -> bool {
        let my_oods = self.ood_list();
        let other_oods = other.ood_list();

        for id in my_oods {
            if !other_oods.contains(id) {
                return false;
            }
        }

        for id in other_oods {
            if !my_oods.contains(id) {
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

    pub fn prev_blob_id(&self) -> &Option<ObjectId> {
        &self.common().prev_blob_id
    }

    pub fn set_prev_blob_id(&mut self, prev_blob_id: Option<ObjectId>) {
        self.common_mut().prev_blob_id = prev_blob_id;
    }
    // pub fn join_member(
    //     &self,
    //     member_id: &ObjectId,
    //     private_key: &PrivateKey,
    // ) -> BuckyResult<&GroupJoinSignature> {
    //     unimplemented!()
    // }

    // pub fn verify(
    //     &self,
    //     signature: &GroupJoinSignature,
    //     member_id: &ObjectId,
    //     public_key: &PublicKey,
    // ) -> BuckyResult<bool> {
    //     unimplemented!()
    // }

    // pub fn verify_member(
    //     &self,
    //     member_id: &ObjectId,
    //     is_admin: bool,
    //     public_key: &PublicKey,
    // ) -> BuckyResult<bool> {
    //     unimplemented!()
    // }

    // pub fn verify_members(
    //     &self,
    //     members: &[(ObjectId, PublicKey)],
    //     is_admin: bool,
    // ) -> BuckyResult<bool> {
    //     unimplemented!()
    // }

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
            GroupMemberScope::Admin => self.admins().iter().map(|m| &m.id).collect::<Vec<_>>(),
            GroupMemberScope::Member => self.members().iter().map(|m| &m.id).collect::<Vec<_>>(),
            GroupMemberScope::All => [
                self.admins().iter().map(|m| &m.id).collect::<Vec<_>>(),
                self.members().iter().map(|m| &m.id).collect::<Vec<_>>(),
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
        let mut oods = self
            .ood_list()
            .iter()
            .map(|id| id.object_id())
            .collect::<Vec<_>>();
        oods.sort_unstable_by(|l, r| {
            let dl = l.distance_of(target);
            let dr = r.distance_of(target);
            dl.cmp(&dr)
        });
        oods
    }

    // pub fn group_hash(&self) -> ObjectId {
    //     let mut without_sign = self.clone();
    //     without_sign.common_mut().join_signatures = vec![];

    //     let mut hash = HashValue::default();
    //     let remain = without_sign
    //         .raw_encode(hash.as_mut_slice(), &Some(RawEncodePurpose::Hash))
    //         .unwrap();
    //     assert_eq!(remain.len(), 0);

    //     ObjectId::from_slice_value(&hash.as_slice()[..31])
    // }

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

#[derive(Clone, Debug, Default)]
struct CommonGroupBodyContent {
    name: String,
    icon: Option<FileId>,
    description: String,

    members: Vec<GroupMember>,

    ood_list: Vec<DeviceId>,

    version: u64,
    prev_blob_id: Option<ObjectId>,
}

impl CommonGroupBodyContent {
    fn new(
        name: String,
        icon: Option<FileId>,
        description: String,
        members: Vec<GroupMember>,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            name,
            icon,
            description,
            members,
            ood_list,
            version: 0,
            prev_blob_id: None,
        }
    }
}

impl TryFrom<protos::CommonGroupBodyContent> for CommonGroupBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::CommonGroupBodyContent) -> BuckyResult<Self> {
        let ret = Self {
            name: value.take_name(),
            icon: if value.has_icon() {
                Some(ProtobufCodecHelper::decode_buf(value.take_icon())?)
            } else {
                None
            },
            description: value.take_description(),
            members: ProtobufCodecHelper::decode_value_list(value.take_members())?,
            ood_list: ProtobufCodecHelper::decode_buf_list(value.take_ood_list())?,
            version: value.version,
            prev_blob_id: if value.has_prev_blob_id() {
                Some(ProtobufCodecHelper::decode_buf(value.take_prev_blob_id())?)
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

        ret.name = value.name.clone();
        if let Some(icon) = &value.icon {
            ret.set_icon(icon.to_vec()?);
        }
        ret.description = value.description.clone();

        ret.set_members(ProtobufCodecHelper::encode_nested_list(&value.members)?);
        ret.set_ood_list(ProtobufCodecHelper::encode_buf_list(
            value.ood_list.as_slice(),
        )?);

        ret.version = value.version;
        if let Some(prev_blob_id) = &value.prev_blob_id {
            ret.set_prev_blob_id(prev_blob_id.to_vec()?);
        }

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct SimpleGroupDescContent {
    unique_id: UniqueId,
    founder_id: ObjectId,
    admins: Vec<GroupMember>,
}

impl TryFrom<protos::SimpleGroupDescContent> for SimpleGroupDescContent {
    type Error = BuckyError;

    fn try_from(value: protos::SimpleGroupDescContent) -> BuckyResult<Self> {
        let ret = Self {
            unique_id: ProtobufCodecHelper::decode_buf(value.unique_id)?,
            admins: ProtobufCodecHelper::decode_value_list(value.admins)?,
            founder_id: ProtobufCodecHelper::decode_buf(value.founder_id)?,
        };

        Ok(ret)
    }
}

impl TryFrom<&SimpleGroupDescContent> for protos::SimpleGroupDescContent {
    type Error = BuckyError;

    fn try_from(value: &SimpleGroupDescContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.unique_id = value.unique_id.to_vec()?;
        ret.founder_id = value.founder_id.to_vec()?;
        ret.set_admins(ProtobufCodecHelper::encode_nested_list(&value.admins)?);

        Ok(ret)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimpleGroupBodyContent {
    common: CommonGroupBodyContent,
}

impl SimpleGroupBodyContent {
    fn new(
        name: String,
        icon: Option<FileId>,
        description: String,
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
    founder_id: ObjectId,
}

impl TryFrom<protos::OrgDescContent> for OrgDescContent {
    type Error = BuckyError;

    fn try_from(value: protos::OrgDescContent) -> BuckyResult<Self> {
        let ret = Self {
            unique_id: ProtobufCodecHelper::decode_buf(value.unique_id)?,
            founder_id: ProtobufCodecHelper::decode_buf(value.founder_id)?,
        };

        Ok(ret)
    }
}

impl TryFrom<&OrgDescContent> for protos::OrgDescContent {
    type Error = BuckyError;

    fn try_from(value: &OrgDescContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.unique_id = value.unique_id.to_vec()?;
        ret.founder_id = value.founder_id.to_vec()?;

        Ok(ret)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OrgBodyContent {
    admins: Vec<GroupMember>,
    common: CommonGroupBodyContent,
}

impl OrgBodyContent {
    fn new(
        name: String,
        icon: Option<FileId>,
        description: String,
        admins: Vec<GroupMember>,
        members: Vec<GroupMember>,
        ood_list: Vec<DeviceId>,
    ) -> Self {
        Self {
            common: CommonGroupBodyContent::new(name, icon, description, members, ood_list),
            admins,
        }
    }

    pub fn admins(&self) -> &Vec<GroupMember> {
        &self.admins
    }

    pub fn set_admins(&mut self, admins: Vec<GroupMember>) {
        self.admins = admins;
    }
}

impl TryFrom<protos::OrgBodyContent> for OrgBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::OrgBodyContent) -> BuckyResult<Self> {
        let ret = Self {
            admins: ProtobufCodecHelper::decode_value_list(value.take_admins())?,
            common: ProtobufCodecHelper::decode_value(value.take_common())?,
        };

        Ok(ret)
    }
}

impl TryFrom<&OrgBodyContent> for protos::OrgBodyContent {
    type Error = BuckyError;

    fn try_from(value: &OrgBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_admins(ProtobufCodecHelper::encode_nested_list(&value.admins)?);
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
