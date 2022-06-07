use crate::root_state::*;
use cyfs_base::*;
use cyfs_core::codec::protos::core_objects as protos;
use cyfs_core::*;

use std::convert::{TryFrom, TryInto};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AdminGlobalStateAccessModeData {
    pub category: GlobalStateCategory,
    pub access_mode: GlobalStateAccessMode,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AdminCommand {
    GlobalStateAccessMode(AdminGlobalStateAccessModeData),
}

#[derive(Debug, Clone)]
pub struct AdminDescContent {
    pub target: DeviceId,
    pub cmd: AdminCommand,
}

impl DescContent for AdminDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::Admin as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Debug, Clone)]
pub struct AdminBodyContent {}

impl BodyContent for AdminBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AdminObjectType = NamedObjType<AdminDescContent, AdminBodyContent>;
type AdminObjectBuilder = NamedObjectBuilder<AdminDescContent, AdminBodyContent>;
type AdminObjectDesc = NamedObjectDesc<AdminDescContent>;

pub type AdminObjectId = NamedObjectId<AdminObjectType>;
pub type AdminObject = NamedObjectBase<AdminObjectType>;

impl TryFrom<protos::AdminGlobalStateAccessModeData> for AdminGlobalStateAccessModeData {
    type Error = BuckyError;

    fn try_from(value: protos::AdminGlobalStateAccessModeData) -> BuckyResult<Self> {
        let category = match value.category {
            protos::AdminGlobalStateAccessModeData_Category::RootState => {
                GlobalStateCategory::RootState
            }
            protos::AdminGlobalStateAccessModeData_Category::LocalCache => {
                GlobalStateCategory::LocalCache
            }
        };

        let access_mode = match value.access_mode {
            protos::AdminGlobalStateAccessModeData_AccessMode::Read => GlobalStateAccessMode::Read,
            protos::AdminGlobalStateAccessModeData_AccessMode::Write => {
                GlobalStateAccessMode::Write
            }
        };

        Ok(Self {
            category,
            access_mode,
        })
    }
}

impl TryFrom<&AdminGlobalStateAccessModeData> for protos::AdminGlobalStateAccessModeData {
    type Error = BuckyError;

    fn try_from(value: &AdminGlobalStateAccessModeData) -> BuckyResult<Self> {
        let category = match value.category {
            GlobalStateCategory::RootState => {
                protos::AdminGlobalStateAccessModeData_Category::RootState
            }
            GlobalStateCategory::LocalCache => {
                protos::AdminGlobalStateAccessModeData_Category::LocalCache
            }
        };

        let access_mode = match value.access_mode {
            GlobalStateAccessMode::Read => protos::AdminGlobalStateAccessModeData_AccessMode::Read,
            GlobalStateAccessMode::Write => {
                protos::AdminGlobalStateAccessModeData_AccessMode::Write
            }
        };

        let mut ret = Self::new();
        ret.set_category(category);
        ret.set_access_mode(access_mode);

        Ok(ret)
    }
}

impl_default_protobuf_raw_codec!(AdminGlobalStateAccessModeData);

impl TryFrom<protos::AdminDescContent> for AdminDescContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::AdminDescContent) -> BuckyResult<Self> {
        let cmd = match value.cmd {
            protos::AdminDescContent_Command::GlobalStateAccessMode => {
                if !value.has_global_state_access_mode() {
                    let msg = format!(
                        "invalid AdminDescContent global_state_access_mode field! {:?}",
                        value
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let data =
                    ProtobufCodecHelper::decode_nested_item(value.take_global_state_access_mode())?;
                AdminCommand::GlobalStateAccessMode(data)
            }
        };

        let target = ProtobufCodecHelper::decode_buf(value.take_target())?;

        let ret = Self { target, cmd };

        Ok(ret)
    }
}

impl TryFrom<&AdminDescContent> for protos::AdminDescContent {
    type Error = BuckyError;

    fn try_from(value: &AdminDescContent) -> BuckyResult<Self> {
        let mut ret = protos::AdminDescContent::new();

        match value.cmd {
            AdminCommand::GlobalStateAccessMode(ref data) => {
                let data = data.try_into()?;
                ret.set_global_state_access_mode(data);
            }
        }

        ret.set_target(value.target.to_vec().unwrap());

        Ok(ret)
    }
}

impl_default_protobuf_raw_codec!(AdminDescContent);
impl_empty_protobuf_raw_codec!(AdminBodyContent);

pub trait AdminObj {
    fn create(owner: ObjectId, target: DeviceId, cmd: AdminCommand) -> Self;

    fn target(&self) -> &DeviceId;

    fn into_command(self) -> AdminCommand;
}

impl AdminObj for AdminObject {
    fn create(owner: ObjectId, target: DeviceId, cmd: AdminCommand) -> Self {
        let desc = AdminDescContent { target, cmd };

        let body = AdminBodyContent {};

        AdminObjectBuilder::new(desc, body).owner(owner).build()
    }

    fn target(&self) -> &DeviceId {
        &self.desc().content().target
    }

    fn into_command(self) -> AdminCommand {
        self.into_desc().into_content().cmd
    }
}


#[cfg(test)]
mod test {
    use cyfs_base::*;
    use crate::*;

    use std::str::FromStr;

    #[test]
    fn test_object() {
        let data = AdminGlobalStateAccessModeData {
            category: GlobalStateCategory::RootState,
            access_mode: GlobalStateAccessMode::Read,
        };

        let cmd = AdminCommand::GlobalStateAccessMode(data);

        let target = DeviceId::from_str("5aSixgLkHa2NR4vSKJLYLPo5Av6CY3RJeFJegtF5iR1g").unwrap();
        let owner = PeopleId::default();
        let obj = AdminObject::create(owner.into(), target.clone(), cmd.clone());
        let buf = obj.to_vec().unwrap();
        println!("{}", hex::encode(&buf));

        let obj = AdminObject::clone_from_slice(&buf).unwrap();
        assert_eq!(*obj.target(), target);
        let c_cmd = obj.into_command();
        assert_eq!(c_cmd, cmd);
    }
}