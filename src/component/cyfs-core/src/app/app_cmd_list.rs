use crate::app::app_cmd::AppCmd;
use crate::coreobj::CoreObjectType;
use crate::{codec::*, AppCmdObj};
use cyfs_base::*;
use serde::Serialize;
use std::collections::VecDeque;

pub const DEFAULT_CMD_LIST: &str = "default";
pub const CMD_LIST_PATH: &str = "/app/manager/cmd_list";

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::AppCmdListItem)]
pub struct AppCmdListItem {
    pub cmd: AppCmd,
    pub retry_count: u32,
}

impl ProtobufTransform<protos::AppCmdListItem> for AppCmdListItem {
    fn transform(value: protos::AppCmdListItem) -> BuckyResult<Self> {
        let cmd = ProtobufCodecHelper::decode_buf(value.cmd)?;
        Ok(Self {
            cmd,
            retry_count: value.retry_count,
        })
    }
}

impl ProtobufTransform<&AppCmdListItem> for protos::AppCmdListItem {
    fn transform(value: &AppCmdListItem) -> BuckyResult<Self> {
        Ok(Self {
            cmd: value.cmd.to_vec()?,
            retry_count: value.retry_count,
        })
    }
}

impl ObjectFormat for AppCmdListItem {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        ObjectFormatHelper::encode_field(&mut map, "cmd", &self.cmd);
        JsonCodecHelper::encode_string_field(&mut map, "retry_count", &self.retry_count);

        serde_json::Value::Object(map)
    }
}

#[derive(Clone, ProtobufEncode, ProtobufDecode, ProtobufTransformType)]
#[cyfs_protobuf_type(crate::codec::protos::AppCmdListDesc)]
pub struct AppCmdListDesc {
    id: String,
    list: VecDeque<AppCmdListItem>,
}

impl DescContent for AppCmdListDesc {
    fn obj_type() -> u16 {
        CoreObjectType::AppList as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

impl ProtobufTransform<protos::AppCmdListDesc> for AppCmdListDesc {
    fn transform(value: protos::AppCmdListDesc) -> BuckyResult<Self> {
        let mut list: VecDeque<AppCmdListItem> = VecDeque::new();
        for item in value.list {
            let cmd = ProtobufCodecHelper::decode_buf(item.cmd)?;
            let retry_count = item.retry_count;
            list.push_back(AppCmdListItem { cmd, retry_count });
        }

        Ok(Self { id: value.id, list })
    }
}

impl ProtobufTransform<&AppCmdListDesc> for protos::AppCmdListDesc {
    fn transform(value: &AppCmdListDesc) -> BuckyResult<Self> {
        let mut list = Vec::new();
        for v in value.list.iter() {
            let item = protos::AppCmdListItem {
                cmd: v.cmd.to_vec()?,
                retry_count: v.retry_count,
            };
            list.push(item);
        }

        Ok(Self {
            id: value.id.to_owned(),
            list: list.into(),
        })
    }
}

impl ObjectFormat for AppCmdListDesc {
    fn format_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        ObjectFormatHelper::encode_array(&mut map, "list", &self.list);
        JsonCodecHelper::encode_string_field(&mut map, "id", &self.id);

        serde_json::Value::Object(map)
    }
}

#[derive(Clone, Default, ProtobufEmptyEncode, ProtobufEmptyDecode, Serialize)]
pub struct AppCmdListBody {}

impl BodyContent for AppCmdListBody {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppCmdListType = NamedObjType<AppCmdListDesc, AppCmdListBody>;
type AppCmdListBuilder = NamedObjectBuilder<AppCmdListDesc, AppCmdListBody>;

pub type AppCmdListId = NamedObjectId<AppCmdListType>;
pub type AppCmdList = NamedObjectBase<AppCmdListType>;

pub trait AppCmdListObj {
    fn create(owner: ObjectId, id: &str) -> Self;
    fn push_back(&mut self, cmd: AppCmd, retry_count: u32);
    fn push_front(&mut self, cmd: AppCmd, retry_count: u32);
    fn pop_front(&mut self) -> Option<AppCmdListItem>;
    fn front(&self) -> Option<&AppCmdListItem>;
    fn clear(&mut self);
    fn size(&self) -> usize;
    fn id(&self) -> &str;
    fn list(&self) -> &VecDeque<AppCmdListItem>;
    fn output(&self) -> String;
}

impl AppCmdListObj for AppCmdList {
    fn create(owner: ObjectId, id: &str) -> Self {
        let desc = AppCmdListDesc {
            id: id.to_owned(),
            list: VecDeque::new(),
        };
        AppCmdListBuilder::new(desc, AppCmdListBody {})
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn push_back(&mut self, cmd: AppCmd, retry_count: u32) {
        self.desc_mut()
            .content_mut()
            .list
            .push_back(AppCmdListItem { cmd, retry_count });
    }

    fn push_front(&mut self, cmd: AppCmd, retry_count: u32) {
        self.desc_mut()
            .content_mut()
            .list
            .push_front(AppCmdListItem { cmd, retry_count });
    }

    fn pop_front(&mut self) -> Option<AppCmdListItem> {
        let ret = self.desc_mut().content_mut().list.pop_front();
        ret
    }

    fn front(&self) -> Option<&AppCmdListItem> {
        self.desc().content().list.front()
    }

    fn clear(&mut self) {
        self.desc_mut().content_mut().list.clear();
    }

    fn size(&self) -> usize {
        self.desc().content().list.len()
    }

    fn id(&self) -> &str {
        &self.desc().content().id
    }

    fn list(&self) -> &VecDeque<AppCmdListItem> {
        &self.desc().content().list
    }

    fn output(&self) -> String {
        let list = self.list();
        let mut output = format!(
            "[AppCmdList] id: {}, size:{}",
            self.desc().calculate_id(),
            list.len()
        );
        for item in list {
            let cmd = &item.cmd;
            output = format!("{}\n{}", output, cmd.output());
        }
        output
    }
}
