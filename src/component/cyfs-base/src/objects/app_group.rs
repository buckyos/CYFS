use crate::*;
use crate::objects::*;
use crate::{RawDecode, RawEncode, RawEncodePurpose};

use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct AppGroupDescContent {}

impl DescContent for AppGroupDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::AppGroup.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct AppGroupBodyContent {}

impl BodyContent for AppGroupBodyContent {}

pub type AppGroupType = NamedObjType<AppGroupDescContent, AppGroupBodyContent>;
pub type AppGroupBuilder = NamedObjectBuilder<AppGroupDescContent, AppGroupBodyContent>;

pub type AppGroupDesc = NamedObjectDesc<AppGroupDescContent>;
pub type AppGroupId = NamedObjectId<AppGroupType>;
pub type AppGroup = NamedObjectBase<AppGroupType>;

impl AppGroupDesc {
    pub fn action_id(&self) -> AppGroupId {
        AppGroupId::try_from(self.calculate_id()).unwrap()
    }
}

impl AppGroup {
    pub fn new() -> AppGroupBuilder {
        let desc_content = AppGroupDescContent {};
        let body_content = AppGroupBodyContent {};
        AppGroupBuilder::new(desc_content, body_content)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    //use std::path::Path;

    #[test]
    fn app_group() {
        let action = AppGroup::new().build();

        // let p = Path::new("f:\\temp\\app_group.obj");
        // if p.parent().unwrap().exists() {
        //     action.clone().encode_to_file(p, false);
        // }

        let buf = action.to_vec().unwrap();
        let _obj = AppGroup::clone_from_slice(&buf).unwrap();
    }
}
