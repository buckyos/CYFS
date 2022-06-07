use crate::*;

use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ActionDescContent {}

impl DescContent for ActionDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Action.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ActionBodyContent {}

impl BodyContent for ActionBodyContent {}

pub type ActionType = NamedObjType<ActionDescContent, ActionBodyContent>;
pub type ActionBuilder = NamedObjectBuilder<ActionDescContent, ActionBodyContent>;

pub type ActionDesc = NamedObjectDesc<ActionDescContent>;
pub type ActionId = NamedObjectId<ActionType>;
pub type Action = NamedObjectBase<ActionType>;

impl ActionDesc {
    pub fn action_id(&self) -> ActionId {
        ActionId::try_from(self.calculate_id()).unwrap()
    }
}

impl Action {
    pub fn new() -> ActionBuilder {
        let desc_content = ActionDescContent {};
        let body_content = ActionBodyContent {};
        ActionBuilder::new(desc_content, body_content)
    }
}

#[cfg(test)]
mod test {
    use crate::{Action, RawConvertTo, RawFrom};

    #[test]
    fn action() {
        let action = Action::new().build();

        // let p = Path::new("f:\\temp\\action.obj");
        // if p.parent().unwrap().exists() {
        //     action.clone().encode_to_file(p, false);
        // }

        let buf = action.to_vec().unwrap();
        let _obj = Action::clone_from_slice(&buf).unwrap();
    }
}
