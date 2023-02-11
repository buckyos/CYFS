use crate::app::app_cmd::AppQuota;
use crate::codec::*;
use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use cyfs_base::*;
use serde::Serialize;

use core::fmt;
use int_enum::IntEnum;
use std::collections::HashMap;

pub const APP_LOCAL_STATUS_MAIN_PATH: &str = "/app_local_status";

#[derive(Clone, Copy, Eq, PartialEq, Debug, IntEnum, Serialize)]
#[repr(u8)]
pub enum AppLocalStatusCode {
    Init = 0,
    Installing = 1, //安装成功进入Stop或者NoService，所以没有Installed
    InstallFailed = 3,

    NoService = 4,
    Stopping = 5,
    Stop = 6,
    StopFailed = 7,

    Starting = 8,
    Running = 9,
    StartFailed = 10,

    Uninstalling = 11,
    UninstallFailed = 12,
    Uninstalled = 13,

    //Removed = 14,      //已经删除
    RunException = 15, //运行异常

    //setpermissioning? upgrading? setversion?
    ErrStatus = 255,
}

impl fmt::Display for AppLocalStatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &AppLocalStatusCode::Init => write!(f, "Init"),
            &AppLocalStatusCode::Installing => write!(f, "Installing"),
            &AppLocalStatusCode::InstallFailed => write!(f, "InstallFailed"),
            &AppLocalStatusCode::NoService => write!(f, "NoService"),
            &AppLocalStatusCode::Stopping => write!(f, "Stopping"),
            &AppLocalStatusCode::Stop => write!(f, "Stop"),
            &AppLocalStatusCode::StopFailed => write!(f, "StopFailed"),
            &AppLocalStatusCode::Starting => write!(f, "Starting"),
            &AppLocalStatusCode::Running => write!(f, "Running"),
            &AppLocalStatusCode::StartFailed => write!(f, "StartFailed"),
            &AppLocalStatusCode::Uninstalling => write!(f, "Uninstalling"),
            &AppLocalStatusCode::UninstallFailed => write!(f, "UninstallFailed"),
            &AppLocalStatusCode::Uninstalled => write!(f, "Uninstalled"),
            //&AppLocalStatusCode::Removed => write!(f, "Removed"),
            &AppLocalStatusCode::RunException => write!(f, "RunException"),
            &AppLocalStatusCode::ErrStatus => write!(f, "ErrStatus"),
        }
    }
}

impl std::convert::From<u8> for AppLocalStatusCode {
    fn from(value: u8) -> Self {
        match Self::from_int(value) {
            Ok(v) => v,
            Err(e) => {
                error!("unknown AppLocalStatusCode value: {} {}", value, e);
                Self::ErrStatus
            }
        }
    }
}

/*更小粒度的错误，比如安装失败时，可以细分错误：检查兼容性失败，下载失败，等等*/
#[derive(Clone, Copy, Eq, PartialEq, Debug, IntEnum, Serialize)]
#[repr(u8)]
pub enum SubErrorCode {
    None = 0,
    Incompatible = 1,
    NoVersion = 2,
    DownloadFailed = 3,
    DockerFailed = 4,
    CommondFailed = 5,
    AppNotFound = 6,
    QueryPermissionError = 7,
    LoadFailed = 8,
    RemoveFailed = 9,
    AssignContainerIpFailed = 10,
    RegisterAppFailed = 11,
    PubDirFailed = 12,
    Unknown = 255,
}

impl fmt::Display for SubErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &SubErrorCode::None => write!(f, "None"),
            &SubErrorCode::Incompatible => write!(f, "Incompatible"),
            &SubErrorCode::NoVersion => write!(f, "NoVersion"),
            &SubErrorCode::DownloadFailed => write!(f, "DownloadFailed"),
            &SubErrorCode::DockerFailed => write!(f, "DockerFailed"),
            &SubErrorCode::CommondFailed => write!(f, "CommondFailed"),
            &SubErrorCode::AppNotFound => write!(f, "AppNotFound"),
            &SubErrorCode::QueryPermissionError => write!(f, "QueryPermissionError"),
            &SubErrorCode::LoadFailed => write!(f, "LoadFailed"),
            &SubErrorCode::RemoveFailed => write!(f, "RemoveFailed"),
            &SubErrorCode::AssignContainerIpFailed => write!(f, "AssignContainerIpFailed"),
            &SubErrorCode::RegisterAppFailed => write!(f, "RegisterAppFailed"),
            &SubErrorCode::PubDirFailed => write!(f, "PubDirFailed"),
            &SubErrorCode::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::convert::From<u8> for SubErrorCode {
    fn from(value: u8) -> Self {
        match Self::from_int(value) {
            Ok(v) => v,
            Err(e) => {
                error!("unknown SubErrorCode value: {} {}", value, e);
                Self::Unknown
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, IntEnum, Serialize)]
#[repr(u8)]
pub enum PermissionState {
    Unhandled = 0, //未处理
    Blocked = 1,   //阻止
    Granted = 2,   //已授权
}

impl fmt::Display for PermissionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &PermissionState::Unhandled => write!(f, "Unhandled"),
            &PermissionState::Blocked => write!(f, "Blocked"),
            &PermissionState::Granted => write!(f, "Granted"),
        }
    }
}

impl std::convert::From<u8> for PermissionState {
    fn from(value: u8) -> Self {
        match Self::from_int(value) {
            Ok(v) => v,
            Err(e) => {
                error!("unknown AppLocalStatusCode value: {} {}", value, e);
                Self::Unhandled
            }
        }
    }
}

//state表示是否通过 -1:未处理,0:不同意，1：同意
#[derive(Clone, Debug, Serialize)]
pub struct PermissionNode {
    reason: String,
    state: PermissionState,
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppLocalStatusDesc)]
pub struct AppLocalStatusDesc {
    id: DecAppId,
    status: AppLocalStatusCode,
    version: Option<String>,
    web_dir: Option<ObjectId>,
    permissions: HashMap<String, PermissionNode>,
    quota: AppQuota,
    last_status_update_time: u64,
    sub_error: SubErrorCode,
    auto_update: bool,
}
impl DescContent for AppLocalStatusDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppLocalStatus as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl ProtobufTransform<protos::AppLocalStatusDesc> for AppLocalStatusDesc {
    fn transform(value: protos::AppLocalStatusDesc) -> BuckyResult<Self> {
        let mut permissions = HashMap::new();
        for permission_item in value.permissions {
            let permission = permission_item.permission;
            let reason = permission_item.reason;
            let state = ProtobufCodecHelper::decode_value(permission_item.state as u8)?;
            permissions.insert(permission, PermissionNode { reason, state });
        }

        let quota = ProtobufTransform::transform(value.quota.unwrap())?;

        let mut ret = Self {
            id: ProtobufCodecHelper::decode_buf(value.id)?,
            status: ProtobufCodecHelper::decode_value(value.status as u8)?,
            version: None,
            web_dir: None,
            permissions,
            quota,
            last_status_update_time: value.last_status_update_time.parse::<u64>()?,
            sub_error: ProtobufCodecHelper::decode_value(value.sub_error as u8)?,
            auto_update: value.auto_update,
        };
        if value.version.is_some() {
            ret.version = Some(value.version.unwrap());
        }
        if value.web_dir.is_some() {
            ret.web_dir = Some(ProtobufCodecHelper::decode_buf(value.web_dir.unwrap())?);
        }

        Ok(ret)
    }
}

impl ProtobufTransform<&AppLocalStatusDesc> for protos::AppLocalStatusDesc {
    fn transform(value: &AppLocalStatusDesc) -> BuckyResult<Self> {
        // let quota = protos::AppQuota {
        //     mem: value.quota.mem,
        //     disk_space: value.quota.disk_space,
        //     cpu: value.quota.cpu,
        // };
        let quota = ProtobufTransform::transform(&value.quota)?;
        let mut ret = Self {
            id: vec![],
            status: value.status as u32,
            web_dir: None,
            version: None,
            permissions: vec![],
            quota: Some(quota),
            last_status_update_time: value.last_status_update_time.to_string(),
            sub_error: value.sub_error as u32,
            auto_update: value.auto_update,
        };
        ret.id = value.id.to_vec()?;
        if let Some(dir) = &value.web_dir {
            ret.web_dir = Some(dir.to_vec()?);
        }
        if let Some(version) = &value.version {
            ret.version = Some(version.to_owned());
        }

        let mut permissions = Vec::new();
        for (k, v) in &value.permissions {
            let item = protos::AppPermission {
                permission: k.to_owned(),
                reason: v.reason.to_owned(),
                state: v.state as u32,
            };
            permissions.push(item);
        }
        permissions.sort_by(|left, right| left.permission.partial_cmp(&right.permission).unwrap());
        ret.permissions = permissions.into();

        Ok(ret)
    }
}

type AppLocalStatusType = NamedObjType<AppLocalStatusDesc, AppLocalStatusBody>;
type AppLocalStatusBuilder = NamedObjectBuilder<AppLocalStatusDesc, AppLocalStatusBody>;

pub type AppLocalStatusId = NamedObjectId<AppLocalStatusType>;
pub type AppLocalStatus = NamedObjectBase<AppLocalStatusType>;

pub trait AppLocalStatusObj {
    fn create(owner: ObjectId, id: DecAppId) -> Self;

    fn app_id(&self) -> &DecAppId;
    fn status(&self) -> AppLocalStatusCode;
    fn web_dir(&self) -> Option<&ObjectId>;
    fn version(&self) -> Option<&str>;
    fn permissions(&self) -> &HashMap<String, PermissionNode>;
    fn quota(&self) -> &AppQuota;
    //获取没有被处理过的权限
    fn permission_unhandled(&self) -> Option<HashMap<String, String>>;
    fn last_status_update_time(&self) -> u64;
    fn sub_error(&self) -> SubErrorCode;
    fn auto_update(&self) -> bool;

    fn set_status(&mut self, status: AppLocalStatusCode);
    fn set_web_dir(&mut self, web_dir: Option<ObjectId>);
    fn set_version(&mut self, version: &str);
    //添加权限，如果权限列表里已经存在的会被pass，返回值表示有没有新的权限被添加
    fn add_permissions(&mut self, permissions: &HashMap<String, String>) -> bool;
    //设置已有的权限，返回值表示权限有没有变化（是否与现有权限相同）
    fn set_permissions(&mut self, permissions: &HashMap<String, PermissionState>) -> bool;
    //设置配额，返回值表示配额有没有变化（是否与现有权限相同）
    fn set_quota(&mut self, quota: &AppQuota) -> bool;
    fn set_sub_error(&mut self, code: SubErrorCode);
    //return old auto_update value
    fn set_auto_update(&mut self, auto_update: bool) -> bool;

    fn output(&self) -> String;
}

impl AppLocalStatusObj for AppLocalStatus {
    fn create(owner: ObjectId, id: DecAppId) -> Self {
        //默认配额
        let quota = AppQuota {
            mem: 0,
            disk_space: 0,
            cpu: 0,
        };
        let desc = AppLocalStatusDesc {
            id,
            status: AppLocalStatusCode::Init,
            version: None,
            web_dir: None,
            permissions: HashMap::new(),
            quota,
            last_status_update_time: bucky_time_now(),
            sub_error: SubErrorCode::None,
            auto_update: true,
        };
        let body = AppLocalStatusBody {};
        AppLocalStatusBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn app_id(&self) -> &DecAppId {
        &self.desc().content().id
    }

    fn status(&self) -> AppLocalStatusCode {
        self.desc().content().status
    }

    fn sub_error(&self) -> SubErrorCode {
        self.desc().content().sub_error
    }

    fn auto_update(&self) -> bool {
        self.desc().content().auto_update
    }

    fn last_status_update_time(&self) -> u64 {
        self.desc().content().last_status_update_time
    }

    fn set_status(&mut self, status: AppLocalStatusCode) {
        self.desc_mut().content_mut().status = status;
        self.desc_mut().content_mut().last_status_update_time = bucky_time_now();
    }

    fn web_dir(&self) -> Option<&ObjectId> {
        self.desc().content().web_dir.as_ref()
    }

    fn set_web_dir(&mut self, web_dir: Option<ObjectId>) {
        self.desc_mut().content_mut().web_dir = web_dir;
    }

    fn version(&self) -> Option<&str> {
        self.desc().content().version.as_ref().map(|s| s.as_str())
    }

    fn set_version(&mut self, version: &str) {
        self.desc_mut().content_mut().version = Some(version.to_string());
    }

    fn permissions(&self) -> &HashMap<String, PermissionNode> {
        &self.desc().content().permissions
    }

    fn permission_unhandled(&self) -> Option<HashMap<String, String>> {
        let permissions = self.permissions();
        let mut unhandled = HashMap::new();
        for (k, v) in permissions {
            if v.state == PermissionState::Unhandled {
                unhandled.insert(k.to_string(), v.reason.clone());
            }
        }

        if unhandled.is_empty() {
            None
        } else {
            Some(unhandled)
        }
    }

    fn add_permissions(&mut self, permissions: &HashMap<String, String>) -> bool {
        let cur_permissions = &mut self.desc_mut().content_mut().permissions;
        let mut changed = false;
        for (k, v) in permissions {
            if !cur_permissions.contains_key(k) {
                cur_permissions.insert(
                    k.to_string(),
                    PermissionNode {
                        reason: v.to_string(),
                        state: PermissionState::Unhandled,
                    },
                );
                changed = true;
            }
        }
        changed
    }

    fn set_permissions(&mut self, permissions: &HashMap<String, PermissionState>) -> bool {
        let cur_permissions = &mut self.desc_mut().content_mut().permissions;
        let mut changed = false;
        for (k, v) in permissions {
            if let Some(cur_node) = cur_permissions.get_mut(k) {
                if cur_node.state != *v {
                    cur_node.state = *v;
                    changed = true;
                }
            }
        }
        changed
    }

    fn quota(&self) -> &AppQuota {
        &self.desc().content().quota
    }

    fn set_quota(&mut self, quota: &AppQuota) -> bool {
        let cur_quota = &mut self.desc_mut().content_mut().quota;
        let mut changed = false;
        if cur_quota.mem != quota.mem {
            cur_quota.mem = quota.mem;
            changed = true;
        }
        if cur_quota.disk_space != quota.disk_space {
            cur_quota.disk_space = quota.disk_space;
            changed = true;
        }
        if cur_quota.cpu != quota.cpu {
            cur_quota.cpu = quota.cpu;
            changed = true;
        }
        changed
    }

    fn set_sub_error(&mut self, code: SubErrorCode) {
        self.desc_mut().content_mut().sub_error = code;
    }

    fn set_auto_update(&mut self, auto_update: bool) -> bool {
        let old_value = self.auto_update();
        self.desc_mut().content_mut().auto_update = auto_update;
        old_value
    }

    fn output(&self) -> String {
        let app_id = self.app_id();
        let status = self.status();
        let ver = self.version();
        //let web_dir = self.web_dir();
        let sub_err = self.sub_error();
        let self_id = self.desc().calculate_id();
        let auto_update = self.auto_update();
        format!(
            "[AppLocalStatus] appid:{} statusid:{}, status:{}, ver:{:?}, auto_update:{}, sub err:{}",
            app_id, self_id, status, ver, auto_update, sub_err
        )
    }
}

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct AppLocalStatusBody {}

impl BodyContent for AppLocalStatusBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}
