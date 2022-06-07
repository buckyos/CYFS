use crate::coreobj::CoreObjectType;
use crate::DecAppId;
use cyfs_base::*;
use serde::Serialize;

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppStatusDescContent)]
pub struct AppStatusDescContent {
    id: DecAppId,
}
impl DescContent for AppStatusDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::AppStatus as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, ProtobufEncode, ProtobufDecode, ProtobufTransform, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::AppStatusContent)]
pub struct AppStatusContent {
    version: String,
    status: u8,
}

impl BodyContent for AppStatusContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type AppStatusType = NamedObjType<AppStatusDescContent, AppStatusContent>;
type AppStatusBuilder = NamedObjectBuilder<AppStatusDescContent, AppStatusContent>;
type AppStatusDesc = NamedObjectDesc<AppStatusDescContent>;

pub type AppStatusId = NamedObjectId<AppStatusType>;
pub type AppStatus = NamedObjectBase<AppStatusType>;

pub trait AppStatusObj {
    fn create(owner: ObjectId, id: DecAppId, version: String, status: bool) -> Self;
    fn app_id(&self) -> &DecAppId;
    fn version(&self) -> &str;
    fn status(&self) -> bool;
    fn set_version(&mut self, version: String);
    fn set_status(&mut self, status: bool);
}

impl AppStatusObj for AppStatus {
    fn create(owner: ObjectId, id: DecAppId, version: String, status: bool) -> Self {
        let body = AppStatusContent {
            version,
            status: if status { 1 } else { 0 },
        };
        let desc = AppStatusDescContent { id };
        AppStatusBuilder::new(desc, body)
            .owner(owner)
            .no_create_time()
            .build()
    }

    fn app_id(&self) -> &DecAppId {
        &self.desc().content().id
    }

    fn version(&self) -> &str {
        &self.body_expect("").content().version
    }

    fn status(&self) -> bool {
        self.body_expect("").content().status == 1
    }

    fn set_version(&mut self, version: String) {
        self.body_mut_expect("").content_mut().version = version;
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }

    fn set_status(&mut self, status: bool) {
        self.body_mut_expect("").content_mut().status = if status { 1 } else { 0 };
        self.body_mut_expect("")
            .increase_update_time(bucky_time_now());
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;

    use std::convert::TryFrom;
    use std::str::FromStr;

    #[test]
    fn test() {
        let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();
        println!("will calc dec_app_id...");
        let dec_app_id = DecApp::generate_id(owner, "test-dec-app");
        println!("dec_app_id: {}", dec_app_id);

        let version = "1.0.0.1";

        let dec_id = DecAppId::try_from(dec_app_id.clone()).unwrap();
        let app_status = AppStatus::create(owner.clone(), dec_id.clone(), version.to_owned(), true);

        let id = app_status.desc().calculate_id();
        println!("app_status_id: {}", id);

        let buf = app_status.to_vec().unwrap();
        let app_status2 = AppStatus::clone_from_slice(&buf).unwrap();
        assert_eq!(app_status2.app_id(), &dec_id);
        assert_eq!(app_status2.version(), version);
        assert_eq!(app_status2.status(), true);
        assert_eq!(app_status2.desc().calculate_id(), id);

        let path = cyfs_util::get_app_data_dir("tests");
        std::fs::create_dir_all(&path).unwrap();
        let name = path.join("app_status.desc");
        std::fs::write(&name, buf).unwrap();
    }
}
