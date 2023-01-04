use crate::codec as cyfs_base;
use crate::*;

use std::collections::HashSet;
use std::convert::TryFrom;

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
        conclusion_limit: Option<u32>,
        area: Area,
    ) -> GroupBuilder {
        let desc_content = SimpleGroupDescContent {
            unique_id: UniqueId::create_with_random(),
            conclusion_limit: conclusion_limit.map_or((admins.len() as u32 >> 1) + 1, |n| n),
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

    pub fn version(&self) -> u64 {
        self.common().version
    }

    pub fn set_version(&mut self, version: u64) {
        self.common_mut().version = version;
    }

    pub fn consensus_interval(&self) -> u64 {
        self.common().consensus_interval
    }

    pub fn set_consensus_interval(&mut self, interval: u64) {
        self.common_mut().consensus_interval = interval;
    }

    pub fn join_member(
        &self,
        member_id: &ObjectId,
        private_key: &PrivateKey,
    ) -> BuckyResult<&GroupJoinSignature> {
        unimplemented!()
    }

    pub fn verify(
        &self,
        signature: &GroupJoinSignature,
        member_id: &ObjectId,
        public_key: &PublicKey,
    ) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub fn verify_member(
        &self,
        member_id: &ObjectId,
        is_admin: bool,
        public_key: &PublicKey,
    ) -> BuckyResult<bool> {
        unimplemented!()
    }

    pub fn verify_members(
        &self,
        members: &[(ObjectId, PublicKey)],
        is_admin: bool,
    ) -> BuckyResult<bool> {
        unimplemented!()
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
    pub role: String,
    pub shares: u64,
}

impl GroupMember {
    pub fn from_member_id(id: ObjectId) -> GroupMember {
        GroupMember {
            id,
            title: "".to_string(),
            role: "".to_string(),
            shares: 0,
        }
    }
}

impl TryFrom<protos::GroupMember> for GroupMember {
    type Error = BuckyError;

    fn try_from(mut value: protos::GroupMember) -> BuckyResult<Self> {
        let ret = Self {
            id: ProtobufCodecHelper::decode_buf(value.id)?,
            title: value.title,
            role: value.role,
            shares: value.shares,
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
        ret.role = value.role.clone();
        ret.shares = value.shares;

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct GroupMethodACL {
    pub name: String,
    pub target_dec_id: Option<ObjectId>,
    pub r_path: Option<String>,
    pub min_support_percent: f64,
    pub permissions: String, // ACL-String
}

impl TryFrom<protos::GroupMethodACL> for GroupMethodACL {
    type Error = BuckyError;

    fn try_from(mut value: protos::GroupMethodACL) -> BuckyResult<Self> {
        let ret = Self {
            name: value.take_name(),
            target_dec_id: if value.has_target_dec_id() {
                Some(ProtobufCodecHelper::decode_buf(value.take_target_dec_id())?)
            } else {
                None
            },
            r_path: if value.has_r_path() {
                Some(value.take_r_path())
            } else {
                None
            },
            min_support_percent: value.min_support_percent,
            permissions: value.take_permissions(), // ACL-String
        };

        Ok(ret)
    }
}

impl TryFrom<&GroupMethodACL> for protos::GroupMethodACL {
    type Error = BuckyError;

    fn try_from(value: &GroupMethodACL) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.name = value.name.clone();
        if let Some(dec_id) = &value.target_dec_id {
            ret.set_target_dec_id(dec_id.to_vec()?);
        }
        if let Some(r_path) = &value.r_path {
            ret.set_r_path(r_path.clone());
        }
        ret.min_support_percent = value.min_support_percent;
        ret.permissions = value.permissions.clone();

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct GroupRoleACL {
    pub name: String,
    pub target_dec_id: Option<ObjectId>,
    pub r_path: Option<String>,
    pub method: String,

    pub right_percent: f64,
    pub is_operator: bool,
    pub permissions: String, // ACL-String
}

impl TryFrom<protos::GroupRoleACL> for GroupRoleACL {
    type Error = BuckyError;

    fn try_from(mut value: protos::GroupRoleACL) -> BuckyResult<Self> {
        let ret = Self {
            name: value.take_name(),
            target_dec_id: if value.has_target_dec_id() {
                Some(ProtobufCodecHelper::decode_buf(value.take_target_dec_id())?)
            } else {
                None
            },
            r_path: if value.has_r_path() {
                Some(value.take_r_path())
            } else {
                None
            },
            method: value.take_method(),
            right_percent: value.right_percent,
            is_operator: value.is_operator,
            permissions: value.take_permissions(), // ACL-String
        };

        Ok(ret)
    }
}

impl TryFrom<&GroupRoleACL> for protos::GroupRoleACL {
    type Error = BuckyError;

    fn try_from(value: &GroupRoleACL) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_name(value.name.clone());
        if let Some(dec_id) = &value.target_dec_id {
            ret.set_target_dec_id(dec_id.to_vec()?);
        }
        if let Some(r_path) = &value.r_path {
            ret.set_r_path(r_path.clone());
        }
        ret.method = value.method.clone();
        ret.right_percent = value.right_percent;
        ret.is_operator = value.is_operator;
        ret.permissions = value.permissions.clone();

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct GroupJoinSignature {
    signature: Signature,
    member_id: ObjectId,
    is_admin: bool,
    hash: HashValue,
}

impl TryFrom<protos::GroupJoinSignature> for GroupJoinSignature {
    type Error = BuckyError;

    fn try_from(mut value: protos::GroupJoinSignature) -> BuckyResult<Self> {
        let ret = Self {
            signature: ProtobufCodecHelper::decode_buf(value.signature)?,
            member_id: ProtobufCodecHelper::decode_buf(value.member_id)?,
            is_admin: value.is_admin,
            hash: ProtobufCodecHelper::decode_buf(value.hash)?,
        };

        Ok(ret)
    }
}

impl TryFrom<&GroupJoinSignature> for protos::GroupJoinSignature {
    type Error = BuckyError;

    fn try_from(value: &GroupJoinSignature) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.signature = value.signature.to_vec()?;
        ret.member_id = value.member_id.to_vec()?;
        ret.is_admin = value.is_admin;
        ret.hash = value.hash.to_vec()?;

        Ok(ret)
    }
}

#[derive(Clone, Debug, Default)]
struct CommonGroupBodyContent {
    name: String,
    icon: Option<FileId>,
    description: String,

    members: Vec<GroupMember>,
    total_equity: u64,

    // map优化以快速匹配
    method_acls: Vec<GroupMethodACL>,

    role_acls: Vec<GroupRoleACL>,

    ood_list: Vec<DeviceId>,
    history_block_max: u64,
    history_block_lifespan: u64,

    revoked_conclusions: HashSet<ObjectId>,

    version: u64,
    consensus_interval: u64,
    join_signatures: Vec<GroupJoinSignature>,
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
            total_equity: 0,
            method_acls: vec![],
            role_acls: vec![],
            ood_list,
            history_block_max: 0,
            history_block_lifespan: 0,
            revoked_conclusions: HashSet::default(),
            version: 0,
            consensus_interval: 0,
            join_signatures: vec![],
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
            total_equity: value.total_equity,
            method_acls: ProtobufCodecHelper::decode_value_list(value.take_method_acls())?,
            role_acls: ProtobufCodecHelper::decode_value_list(value.take_role_acls())?,
            ood_list: ProtobufCodecHelper::decode_buf_list(value.take_ood_list())?,
            history_block_max: value.get_history_block_max(),
            history_block_lifespan: value.get_history_block_lifespan(),
            revoked_conclusions: HashSet::from_iter(ProtobufCodecHelper::decode_buf_list(
                value.take_revoked_conclusions(),
            )?),
            version: value.version,
            consensus_interval: if value.has_consensus_interval() {
                value.get_consensus_interval()
            } else {
                0
            },
            join_signatures: ProtobufCodecHelper::decode_value_list(
                value.take_join_signatures().into_vec(),
            )?,
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
        ret.total_equity = value.total_equity;
        ret.set_method_acls(ProtobufCodecHelper::encode_nested_list(&value.method_acls)?);
        ret.set_role_acls(ProtobufCodecHelper::encode_nested_list(&value.role_acls)?);
        ret.set_ood_list(ProtobufCodecHelper::encode_buf_list(
            value.ood_list.as_slice(),
        )?);
        if value.history_block_max > 0 {
            ret.set_history_block_max(value.history_block_max);
        }
        if value.history_block_lifespan > 0 {
            ret.set_history_block_lifespan(value.history_block_lifespan);
        }
        ret.set_revoked_conclusions(ProtobufCodecHelper::encode_buf_list(
            value.revoked_conclusions.to_vec()?.as_slice(),
        )?);
        ret.version = value.version;
        if value.consensus_interval > 0 {
            ret.set_consensus_interval(value.consensus_interval);
        }
        ret.set_join_signatures(ProtobufCodecHelper::encode_nested_list(
            &value.join_signatures,
        )?);

        Ok(ret)
    }
}

#[derive(Clone, Debug)]
pub struct SimpleGroupDescContent {
    unique_id: UniqueId,
    founder_id: ObjectId,
    admins: Vec<GroupMember>,
    conclusion_limit: u32,
}

impl TryFrom<protos::SimpleGroupDescContent> for SimpleGroupDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::SimpleGroupDescContent) -> BuckyResult<Self> {
        let ret = Self {
            unique_id: ProtobufCodecHelper::decode_buf(value.unique_id)?,
            admins: ProtobufCodecHelper::decode_value_list(value.admins)?,
            conclusion_limit: value.conclusion_limit,
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
        ret.conclusion_limit = value.conclusion_limit;

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

    fn try_from(mut value: protos::OrgDescContent) -> BuckyResult<Self> {
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
    token_contract: Option<ObjectId>,
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
            token_contract: None,
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
            token_contract: if value.has_token_contract() {
                Some(ProtobufCodecHelper::decode_buf(
                    value.take_token_contract(),
                )?)
            } else {
                None
            },
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

        if let Some(token_contract) = value.token_contract {
            ret.set_token_contract(token_contract.to_vec()?);
        }

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
