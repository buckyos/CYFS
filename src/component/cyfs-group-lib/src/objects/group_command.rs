use cyfs_base::{
    BodyContent, BuckyError, BuckyErrorCode, BuckyResult, DescContent, NamedObjType, NamedObject,
    NamedObjectBase, NamedObjectBuilder, NamedObjectId, ObjectId, ProtobufDecode, ProtobufEncode,
    ProtobufTransform, ProtobufTransformType, RawConvertTo, RawDecode, RawEncode, RawEncodePurpose,
    SubDescNone, OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF,
};

use cyfs_core::{CoreObjectType, GroupConsensusBlock, GroupProposal};
use cyfs_lib::NONObjectInfo;

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandDescContent)]
pub struct GroupCommandDescContent {}

impl DescContent for GroupCommandDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::GroupCommand as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    fn debug_info() -> String {
        String::from("GroupCommandDescContent")
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Debug)]
pub enum GroupCommandType {
    NewRPath,
    Execute,
    ExecuteResult,
    Verify,
    Commited,
}

#[derive(Clone, RawEncode, RawDecode)]
pub enum GroupCommandBodyContent {
    NewRPath(GroupCommandNewRPath),
    Execute(GroupCommandExecute),
    ExecuteResult(GroupCommandExecuteResult),
    Verify(GroupCommandVerify),
    Commited(GroupCommandCommited),
}

pub type GroupCommandObjectType = NamedObjType<GroupCommandDescContent, GroupCommandBodyContent>;
pub type GroupCommandBuilder = NamedObjectBuilder<GroupCommandDescContent, GroupCommandBodyContent>;

pub type GroupCommandId = NamedObjectId<GroupCommandObjectType>;
pub type GroupCommand = NamedObjectBase<GroupCommandObjectType>;

impl GroupCommandBodyContent {
    pub fn cmd_type(&self) -> GroupCommandType {
        match self {
            GroupCommandBodyContent::NewRPath(_) => GroupCommandType::NewRPath,
            GroupCommandBodyContent::Execute(_) => GroupCommandType::Execute,
            GroupCommandBodyContent::ExecuteResult(_) => GroupCommandType::ExecuteResult,
            GroupCommandBodyContent::Verify(_) => GroupCommandType::Verify,
            GroupCommandBodyContent::Commited(_) => GroupCommandType::Commited,
        }
    }
}

pub trait GroupCommandObject {
    fn cmd_type(&self) -> GroupCommandType;
    fn into_cmd(self) -> GroupCommandBodyContent;
}

impl GroupCommandObject for GroupCommand {
    fn cmd_type(&self) -> GroupCommandType {
        self.body().as_ref().unwrap().content().cmd_type()
    }

    fn into_cmd(self) -> GroupCommandBodyContent {
        self.into_body().unwrap().into_content()
    }
}

impl BodyContent for GroupCommandBodyContent {
    fn version(&self) -> u8 {
        0
    }

    fn format(&self) -> u8 {
        cyfs_base::OBJECT_CONTENT_CODEC_FORMAT_RAW
    }

    fn debug_info() -> String {
        String::from("GroupCommandBodyContent")
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandNewRPath)]
pub struct GroupCommandNewRPath {
    pub group_id: ObjectId,
    pub rpath: String,
    pub with_block: Option<GroupConsensusBlock>,
}

impl ProtobufTransform<super::codec::protos::GroupCommandNewRPath> for GroupCommandNewRPath {
    fn transform(value: super::codec::protos::GroupCommandNewRPath) -> BuckyResult<Self> {
        Ok(Self {
            group_id: ObjectId::raw_decode(value.group_id.as_slice())?.0,
            with_block: match value.with_block.as_ref() {
                Some(buf) => Some(GroupConsensusBlock::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            rpath: value.rpath,
        })
    }
}

impl ProtobufTransform<&GroupCommandNewRPath> for super::codec::protos::GroupCommandNewRPath {
    fn transform(value: &GroupCommandNewRPath) -> BuckyResult<Self> {
        Ok(Self {
            group_id: value.group_id.to_vec()?,
            rpath: value.rpath.clone(),
            with_block: match value.with_block.as_ref() {
                Some(block) => Some(block.to_vec()?),
                None => None,
            },
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandExecute)]
pub struct GroupCommandExecute {
    pub proposal: GroupProposal,
    pub prev_state_id: Option<ObjectId>,
}

impl ProtobufTransform<super::codec::protos::GroupCommandExecute> for GroupCommandExecute {
    fn transform(value: super::codec::protos::GroupCommandExecute) -> BuckyResult<Self> {
        Ok(Self {
            prev_state_id: match value.prev_state_id.as_ref() {
                Some(buf) => Some(ObjectId::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            proposal: GroupProposal::raw_decode(value.proposal.as_slice())?.0,
        })
    }
}

impl ProtobufTransform<&GroupCommandExecute> for super::codec::protos::GroupCommandExecute {
    fn transform(value: &GroupCommandExecute) -> BuckyResult<Self> {
        Ok(Self {
            proposal: value.proposal.to_vec()?,
            prev_state_id: match value.prev_state_id.as_ref() {
                Some(prev_state_id) => Some(prev_state_id.to_vec()?),
                None => None,
            },
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandExecuteResult)]
pub struct GroupCommandExecuteResult {
    pub result_state_id: Option<ObjectId>,
    pub receipt: Option<NONObjectInfo>,
    pub context: Option<Vec<u8>>,
}

impl ProtobufTransform<super::codec::protos::GroupCommandExecuteResult>
    for GroupCommandExecuteResult
{
    fn transform(value: super::codec::protos::GroupCommandExecuteResult) -> BuckyResult<Self> {
        Ok(Self {
            result_state_id: match value.result_state_id.as_ref() {
                Some(result_state_id) => Some(ObjectId::raw_decode(result_state_id.as_slice())?.0),
                None => None,
            },
            receipt: match value.receipt.as_ref() {
                Some(buf) => Some(NONObjectInfo::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            context: value.context,
        })
    }
}

impl ProtobufTransform<&GroupCommandExecuteResult>
    for super::codec::protos::GroupCommandExecuteResult
{
    fn transform(value: &GroupCommandExecuteResult) -> BuckyResult<Self> {
        Ok(Self {
            result_state_id: match value.result_state_id.as_ref() {
                Some(result_state_id) => Some(result_state_id.to_vec()?),
                None => None,
            },
            receipt: match value.receipt.as_ref() {
                Some(receipt) => Some(receipt.to_vec()?),
                None => None,
            },
            context: value.context.clone(),
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandVerify)]
pub struct GroupCommandVerify {
    pub proposal: GroupProposal,
    pub prev_state_id: Option<ObjectId>,
    pub result_state_id: Option<ObjectId>,
    pub receipt: Option<NONObjectInfo>,
    pub context: Option<Vec<u8>>,
}

impl ProtobufTransform<super::codec::protos::GroupCommandVerify> for GroupCommandVerify {
    fn transform(value: super::codec::protos::GroupCommandVerify) -> BuckyResult<Self> {
        Ok(Self {
            prev_state_id: match value.prev_state_id {
                Some(buf) => Some(ObjectId::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            proposal: GroupProposal::raw_decode(value.proposal.as_slice())?.0,
            result_state_id: match value.result_state_id {
                Some(buf) => Some(ObjectId::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            receipt: match value.receipt {
                Some(buf) => Some(NONObjectInfo::raw_decode(buf.as_slice())?.0),
                None => None,
            },
            context: value.context,
        })
    }
}

impl ProtobufTransform<&GroupCommandVerify> for super::codec::protos::GroupCommandVerify {
    fn transform(value: &GroupCommandVerify) -> BuckyResult<Self> {
        Ok(Self {
            proposal: value.proposal.to_vec()?,
            prev_state_id: match value.prev_state_id.as_ref() {
                Some(prev_state_id) => Some(prev_state_id.to_vec()?),
                None => None,
            },
            result_state_id: match value.result_state_id.as_ref() {
                Some(result_state_id) => Some(result_state_id.to_vec()?),
                None => None,
            },
            receipt: match value.receipt.as_ref() {
                Some(receipt) => Some(receipt.to_vec()?),
                None => None,
            },
            context: value.context.clone(),
        })
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(super::codec::protos::GroupCommandCommited)]
pub struct GroupCommandCommited {
    pub prev_state_id: Option<ObjectId>,
    pub block: GroupConsensusBlock,
}

impl ProtobufTransform<super::codec::protos::GroupCommandCommited> for GroupCommandCommited {
    fn transform(value: super::codec::protos::GroupCommandCommited) -> BuckyResult<Self> {
        Ok(Self {
            prev_state_id: match value.prev_state_id.as_ref() {
                Some(prev_state_id) => Some(ObjectId::raw_decode(prev_state_id.as_slice())?.0),
                None => None,
            },
            block: GroupConsensusBlock::raw_decode(&value.block.as_slice())?.0,
        })
    }
}

impl ProtobufTransform<&GroupCommandCommited> for super::codec::protos::GroupCommandCommited {
    fn transform(value: &GroupCommandCommited) -> BuckyResult<Self> {
        Ok(Self {
            prev_state_id: match value.prev_state_id.as_ref() {
                Some(prev_state_id) => Some(prev_state_id.to_vec()?),
                None => None,
            },
            block: value.block.to_vec()?,
        })
    }
}

impl From<GroupCommandNewRPath> for GroupCommand {
    fn from(cmd: GroupCommandNewRPath) -> Self {
        let desc = GroupCommandDescContent {};
        let body = GroupCommandBodyContent::NewRPath(cmd);
        GroupCommandBuilder::new(desc, body).build()
    }
}

impl From<GroupCommandExecute> for GroupCommand {
    fn from(cmd: GroupCommandExecute) -> Self {
        let desc = GroupCommandDescContent {};
        let body = GroupCommandBodyContent::Execute(cmd);
        GroupCommandBuilder::new(desc, body).build()
    }
}

impl From<GroupCommandExecuteResult> for GroupCommand {
    fn from(cmd: GroupCommandExecuteResult) -> Self {
        let desc = GroupCommandDescContent {};
        let body = GroupCommandBodyContent::ExecuteResult(cmd);
        GroupCommandBuilder::new(desc, body).build()
    }
}

impl From<GroupCommandVerify> for GroupCommand {
    fn from(cmd: GroupCommandVerify) -> Self {
        let desc = GroupCommandDescContent {};
        let body = GroupCommandBodyContent::Verify(cmd);
        GroupCommandBuilder::new(desc, body).build()
    }
}

impl From<GroupCommandCommited> for GroupCommand {
    fn from(cmd: GroupCommandCommited) -> Self {
        let desc = GroupCommandDescContent {};
        let body = GroupCommandBodyContent::Commited(cmd);
        GroupCommandBuilder::new(desc, body).build()
    }
}

impl TryInto<GroupCommandNewRPath> for GroupCommand {
    type Error = BuckyError;

    fn try_into(self) -> Result<GroupCommandNewRPath, Self::Error> {
        let cmd_type = self.cmd_type();
        match self.into_cmd() {
            GroupCommandBodyContent::NewRPath(cmd) => Ok(cmd),
            _ => Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("is {:?}, expect NewRPath", cmd_type),
            )),
        }
    }
}

impl TryInto<GroupCommandExecute> for GroupCommand {
    type Error = BuckyError;

    fn try_into(self) -> Result<GroupCommandExecute, Self::Error> {
        let cmd_type = self.cmd_type();
        match self.into_cmd() {
            GroupCommandBodyContent::Execute(cmd) => Ok(cmd),
            _ => Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("is {:?}, expect Execute", cmd_type),
            )),
        }
    }
}

impl TryInto<GroupCommandExecuteResult> for GroupCommand {
    type Error = BuckyError;

    fn try_into(self) -> Result<GroupCommandExecuteResult, Self::Error> {
        let cmd_type = self.cmd_type();
        match self.into_cmd() {
            GroupCommandBodyContent::ExecuteResult(cmd) => Ok(cmd),
            _ => Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("is {:?}, expect ExecuteResult", cmd_type),
            )),
        }
    }
}

impl TryInto<GroupCommandVerify> for GroupCommand {
    type Error = BuckyError;

    fn try_into(self) -> Result<GroupCommandVerify, Self::Error> {
        let cmd_type = self.cmd_type();
        match self.into_cmd() {
            GroupCommandBodyContent::Verify(cmd) => Ok(cmd),
            _ => Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("is {:?}, expect Verify", cmd_type),
            )),
        }
    }
}

impl TryInto<GroupCommandCommited> for GroupCommand {
    type Error = BuckyError;

    fn try_into(self) -> Result<GroupCommandCommited, Self::Error> {
        let cmd_type = self.cmd_type();
        match self.into_cmd() {
            GroupCommandBodyContent::Commited(cmd) => Ok(cmd),
            _ => Err(BuckyError::new(
                BuckyErrorCode::Unmatch,
                format!("is {:?}, expect Commited", cmd_type),
            )),
        }
    }
}
