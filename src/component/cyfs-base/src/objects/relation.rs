use crate::*;

use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct RelationDescContent {}

impl DescContent for RelationDescContent {
    fn obj_type() -> u16 {
        ObjectTypeCode::Relation.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct RelationBodyContent {}

impl BodyContent for RelationBodyContent {}

pub type RelationType = NamedObjType<RelationDescContent, RelationBodyContent>;
pub type RelationBuilder = NamedObjectBuilder<RelationDescContent, RelationBodyContent>;

pub type RelationDesc = NamedObjectDesc<RelationDescContent>;
pub type RelationId = NamedObjectId<RelationType>;
pub type Relation = NamedObjectBase<RelationType>;

impl RelationDesc {
    pub fn relation_id(&self) -> RelationId {
        RelationId::try_from(self.calculate_id()).unwrap()
    }
}

impl Relation {
    pub fn new() -> RelationBuilder {
        let desc_content = RelationDescContent {};
        let body_content = RelationBodyContent {};
        RelationBuilder::new(desc_content, body_content)
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn relation() {
        let obj = Relation::new().build();

        // let p = Path::new("f:\\temp\\relation.obj");
        // if p.parent().unwrap().exists() {
        //     obj.clone().encode_to_file(p, false);
        // }

        let buf = obj.to_vec().unwrap();
        let decode_obj = Relation::clone_from_slice(&buf).unwrap();

        assert!(obj.desc().relation_id() == decode_obj.desc().relation_id());
    }
}
