use crate::coreobj::CoreObjectType;
use cyfs_base::*;
use std::collections::HashMap;

#[derive(Clone, Debug, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::DecIpInfo)]
pub struct DecIpInfo {
    pub name: String,
    pub ip: String,
}

#[derive(Clone, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::DecAclInfo)]
pub struct DecAclInfo {
    pub name: String,
    pub acl_info: HashMap<String, bool>,
}

#[derive(Clone, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::RegisterDec)]
pub struct RegisterDec {
    pub docker_gateway_ip: String,
    pub dec_list: HashMap<String, DecIpInfo>,
}

#[derive(Clone, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::UnregisterDec)]
pub struct UnregisterDec {
    pub dec_list: HashMap<String, String>,
}

#[derive(Clone, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::ModifyAcl)]
pub struct ModifyAcl {
    pub dec_list: HashMap<String, DecAclInfo>,
}

#[derive(Clone, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::app_manager_action_desc::AppManagerActionEnum)]
pub enum AppManagerActionEnum {
    RegisterDec(RegisterDec),
    UnregisterDec(UnregisterDec),
    ModifyAcl(ModifyAcl),
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::AppManagerActionDesc)]
pub struct AppManagerActionDesc {
    app_manager_action_enum: AppManagerActionEnum,
}

impl DescContent for AppManagerActionDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppManagerAction as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode)]
pub struct AppManagerActionBody {}

impl BodyContent for AppManagerActionBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppManagerActionType = NamedObjType<AppManagerActionDesc, AppManagerActionBody>;
type AppManagerActionBuilder = NamedObjectBuilder<AppManagerActionDesc, AppManagerActionBody>;

pub type AppManagerActionId = NamedObjectId<AppManagerActionType>;
pub type AppManagerAction = NamedObjectBase<AppManagerActionType>;

pub trait AppManagerActionObj {
    fn create_register_dec(
        owner: ObjectId,
        docker_gateway_ip: String,
        dec_list: HashMap<String, DecIpInfo>,
    ) -> Self;

    fn create_unregister_dec(owner: ObjectId, dec_list: HashMap<String, String>) -> Self;

    fn create_modify_acl(owner: ObjectId, dec_list: HashMap<String, DecAclInfo>) -> Self;

    fn action(&self) -> &AppManagerActionEnum;
}

impl AppManagerActionObj for AppManagerAction {
    fn create_register_dec(
        owner: ObjectId,
        docker_gateway_ip: String,
        dec_list: HashMap<String, DecIpInfo>,
    ) -> Self {
        let action = AppManagerActionEnum::RegisterDec(RegisterDec {
            docker_gateway_ip,
            dec_list,
        });

        let desc = AppManagerActionDesc {
            app_manager_action_enum: action,
        };
        let body = AppManagerActionBody {};
        AppManagerActionBuilder::new(desc, body)
            .owner(owner)
            .build()
    }

    fn create_unregister_dec(owner: ObjectId, dec_list: HashMap<String, String>) -> Self {
        let action = AppManagerActionEnum::UnregisterDec(UnregisterDec { dec_list });

        let desc = AppManagerActionDesc {
            app_manager_action_enum: action,
        };
        let body = AppManagerActionBody {};
        AppManagerActionBuilder::new(desc, body)
            .owner(owner)
            .build()
    }

    fn create_modify_acl(owner: ObjectId, dec_list: HashMap<String, DecAclInfo>) -> Self {
        let action = AppManagerActionEnum::ModifyAcl(ModifyAcl { dec_list });

        let desc = AppManagerActionDesc {
            app_manager_action_enum: action,
        };
        let body = AppManagerActionBody {};
        AppManagerActionBuilder::new(desc, body)
            .owner(owner)
            .build()
    }

    fn action(&self) -> &AppManagerActionEnum {
        &self.desc().content().app_manager_action_enum
    }
}
