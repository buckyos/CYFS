use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use cyfs_base::*;
use serde::Serialize;

pub const APP_SETTING_MAIN_PATH: &str = "/app_setting";

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppSettingDesc)]
pub struct AppSettingDesc {
    id: DecAppId,
    auto_update: bool,
}

impl DescContent for AppSettingDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppSetting as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct AppSettingBody {}

impl BodyContent for AppSettingBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppSettingType = NamedObjType<AppSettingDesc, AppSettingBody>;
type AppSettingBuilder = NamedObjectBuilder<AppSettingDesc, AppSettingBody>;

pub type AppSettingId = NamedObjectId<AppSettingType>;
pub type AppSetting = NamedObjectBase<AppSettingType>;

pub trait AppSettingObj {
    fn create(owner: ObjectId, id: DecAppId) -> Self;
    fn app_id(&self) -> &DecAppId;

    fn auto_update(&self) -> bool;
    fn set_auto_update(&mut self, auto_update: bool);
}

impl AppSettingObj for AppSetting {
    fn create(owner: ObjectId, id: DecAppId) -> Self {
        let body = AppSettingBody {};
        let desc = AppSettingDesc {
            id,
            auto_update: false,
        };
        AppSettingBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn app_id(&self) -> &DecAppId {
        &self.desc().content().id
    }

    fn auto_update(&self) -> bool {
        self.desc().content().auto_update
    }

    fn set_auto_update(&mut self, auto_update: bool) {
        self.desc_mut().content_mut().auto_update = auto_update;
    }
}
