use crate::codec::*;
use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use chrono::{DateTime, Local};
use cyfs_base::*;
use serde::Serialize;
use std::collections::{HashMap};

//AppCmd只提供基本的操作，升级降级可以由客户端拼出来。（先uninstall，再install）
#[derive(Clone, Debug, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::AddApp)]
pub struct AddApp {
    pub app_owner_id: Option<ObjectId>,
}

#[derive(Clone, Debug, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::InstallApp)]
pub struct InstallApp {
    pub ver: String,
    pub run_after_install: bool,
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::ModifyAppPermission)]
pub struct ModifyAppPermission {
    pub permission: HashMap<String, bool>,
}

impl ProtobufTransform<protos::ModifyAppPermission> for ModifyAppPermission {
    fn transform(value: protos::ModifyAppPermission) -> BuckyResult<Self> {
        let mut permission = HashMap::new();
        for item in &value.permission {
            permission.insert(item.key.clone(), item.value);
        }
        Ok(Self {
            permission
        })
    }
}

impl ProtobufTransform<&ModifyAppPermission> for protos::ModifyAppPermission {
    fn transform(value: &ModifyAppPermission) -> BuckyResult<Self> {
        let mut permission = vec![];
        let tree_map : std::collections::BTreeMap<String, bool> = value.permission.clone().into_iter().collect();
        for (key, value) in tree_map {
            permission.push(protos::StringBoolMapItem {key, value})
        }
        Ok(Self {
            permission
        })
    }
}

#[derive(Copy, Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppQuota)]
pub struct AppQuota {
    pub mem: i64,
    pub disk_space: i64,
    pub cpu: i64,
}

impl ProtobufTransform<protos::AppQuota> for AppQuota {
    fn transform(value: protos::AppQuota) -> BuckyResult<Self> {
        Ok(Self {
            mem: value.mem.parse::<i64>()?,
            disk_space: value.disk_space.parse::<i64>()?,
            cpu: value.cpu.parse::<i64>()?,
        })
    }
}

impl ProtobufTransform<&AppQuota> for protos::AppQuota {
    fn transform(value: &AppQuota) -> BuckyResult<Self> {
        Ok(Self {
            mem: value.mem.to_string(),
            disk_space: value.disk_space.to_string(),
            cpu: value.cpu.to_string(),
        })
    }
}

#[derive(Clone, Debug, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::CmdCode)]
pub enum CmdCode {
    Add(AddApp),
    Remove,
    Install(InstallApp),
    Uninstall,
    Start,
    Stop,
    SetPermission(ModifyAppPermission),
    SetQuota(AppQuota),
    Unknown,
}

// impl fmt::Display for CmdCode {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             &CmdCode::Add(add_app) => write!(f, "Init"),
//             &CmdCode::Installing => write!(f, "Installing"),
//             &CmdCode::InstallFailed => write!(f, "InstallFailed"),
//             &CmdCode::NoService => write!(f, "NoService"),
//             &CmdCode::Stopping => write!(f, "Stopping"),
//             &CmdCode::Stop => write!(f, "Stop"),
//             &CmdCode::StopFailed => write!(f, "StopFailed"),
//             &CmdCode::Starting => write!(f, "Starting"),
//             &CmdCode::Running => write!(f, "Running"),
//             &CmdCode::StartFailed => write!(f, "StartFailed"),
//             &CmdCode::Uninstalling => write!(f, "Uninstalling"),
//         }
//     }
// }

impl ProtobufTransform<protos::CmdCode> for CmdCode {
    fn transform(value: protos::CmdCode) -> BuckyResult<Self> {
        let ret = match value.code {
            0 => Self::Add(ProtobufTransform::transform(value.add_app.unwrap())?),
            1 => Self::Remove,
            2 => Self::Install(ProtobufTransform::transform(value.install_app.unwrap())?),
            3 => Self::Uninstall,
            4 => Self::Start,
            5 => Self::Stop,
            6 => Self::SetPermission(ProtobufTransform::transform(value.app_permission.unwrap())?),
            7 => Self::SetQuota(ProtobufTransform::transform(value.app_quota.unwrap())?),
            v @ _ => {
                warn!("unknown app cmd code: {}", v);
                Self::Unknown
            }
        };

        Ok(ret)
    }
}

impl ProtobufTransform<&CmdCode> for protos::CmdCode {
    fn transform(value: &CmdCode) -> BuckyResult<Self> {
        let mut ret = Self {
            code: -1,
            add_app: None,
            install_app: None,
            app_permission: None,
            app_quota: None,
        };
        match value {
            CmdCode::Add(v) => {
                ret.code = 0;
                ret.add_app = Some(ProtobufTransform::transform(v)?);
            }
            CmdCode::Remove => {
                ret.code = 1;
            }
            CmdCode::Install(v) => {
                ret.code = 2;
                ret.install_app = Some(ProtobufTransform::transform(v)?);
            }
            CmdCode::Uninstall => {
                ret.code = 3;
            }
            CmdCode::Start => {
                ret.code = 4;
            }
            CmdCode::Stop => {
                ret.code = 5;
            }
            CmdCode::SetPermission(v) => {
                ret.code = 6;
                ret.app_permission = Some(ProtobufTransform::transform(v)?);
            }
            CmdCode::SetQuota(v) => {
                ret.code = 7;
                ret.app_quota = Some(ProtobufTransform::transform(v)?);
            }
            _ => {
                ret.code = -1;
            }
        }
        Ok(ret)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransform)]
#[cyfs_protobuf_type(crate::codec::protos::AppCmdDesc)]
pub struct AppCmdDesc {
    pub app_id: DecAppId,
    pub cmd_code: CmdCode,
}

impl DescContent for AppCmdDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppCmd as u16
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
pub struct AppCmdBody {}

impl BodyContent for AppCmdBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppCmdType = NamedObjType<AppCmdDesc, AppCmdBody>;
type AppCmdBuilder = NamedObjectBuilder<AppCmdDesc, AppCmdBody>;

pub type AppCmdId = NamedObjectId<AppCmdType>;
pub type AppCmd = NamedObjectBase<AppCmdType>;

pub trait AppCmdObj {
    fn app_id(&self) -> &DecAppId;
    fn cmd(&self) -> &CmdCode;

    fn add(owner: ObjectId, id: DecAppId, app_owner: Option<ObjectId>) -> Self;
    fn remove(owner: ObjectId, id: DecAppId) -> Self;
    fn install(owner: ObjectId, id: DecAppId, version: &str, run_after_install: bool) -> Self;
    fn uninstall(owner: ObjectId, id: DecAppId) -> Self;
    fn start(owner: ObjectId, id: DecAppId) -> Self;
    fn stop(owner: ObjectId, id: DecAppId) -> Self;

    //permission参数： String表示语义路径，bool表示是否授权，如果permission有改变，app会重启并应用新的权限
    fn set_permission(owner: ObjectId, id: DecAppId, permission: ModifyAppPermission) -> Self;

    //quota参数： String表示语义路径，bool表示是否授权，如果permission有改变，app会重启并应用新的权限
    fn set_quota(owner: ObjectId, id: DecAppId, quota: AppQuota) -> Self;

    fn output(&self) -> String;
}

struct AppCmdHelper {}

impl AppCmdHelper {
    fn create(owner: ObjectId, app_id: DecAppId, cmd_code: CmdCode) -> AppCmd {
        let desc = AppCmdDesc { app_id, cmd_code };

        AppCmdBuilder::new(desc, AppCmdBody {})
            .owner(owner)
            .option_create_time(None)
            .build()
    }
}

impl AppCmdObj for AppCmd {
    fn app_id(&self) -> &DecAppId {
        &self.desc().content().app_id
    }

    fn cmd(&self) -> &CmdCode {
        &self.desc().content().cmd_code
    }

    fn add(owner: ObjectId, id: DecAppId, app_owner: Option<ObjectId>) -> Self {
        let cmd = CmdCode::Add(AddApp {
            app_owner_id: app_owner,
        });
        AppCmdHelper::create(owner, id, cmd)
    }

    fn remove(owner: ObjectId, id: DecAppId) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::Remove)
    }

    fn install(owner: ObjectId, id: DecAppId, version: &str, run_after_install: bool) -> Self {
        let cmd = CmdCode::Install(InstallApp {
            ver: version.to_owned(),
            run_after_install,
        });

        AppCmdHelper::create(owner, id, cmd)
    }

    fn uninstall(owner: ObjectId, id: DecAppId) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::Uninstall)
    }

    fn start(owner: ObjectId, id: DecAppId) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::Start)
    }

    fn stop(owner: ObjectId, id: DecAppId) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::Stop)
    }

    fn set_permission(owner: ObjectId, id: DecAppId, permission: ModifyAppPermission) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::SetPermission(permission))
    }

    fn set_quota(owner: ObjectId, id: DecAppId, quota: AppQuota) -> Self {
        AppCmdHelper::create(owner, id, CmdCode::SetQuota(quota))
    }

    fn output(&self) -> String {
        let app_id = self.app_id();
        let cmd_code = self.cmd();
        let create_time = bucky_time_to_system_time(self.desc().create_time());
        let create_time: DateTime<Local> = create_time.into();
        format!(
            "[AppCmd] appid:{}, cmd: {:?}, create time:{}",
            app_id,
            cmd_code,
            create_time.format("[%Y-%m-%d %H:%M:%S.%3f]")
        )
    }
}
