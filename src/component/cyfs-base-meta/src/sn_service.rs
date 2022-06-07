use cyfs_base::*;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNServiceDescContent {
    pub service_type: u8,
    pub price: u64,
}

impl DescContent for SNServiceDescContent {
    fn obj_type() -> u16 {
        // MetaObjectType::SNService as u16
        0_16
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNServiceBodyContent {}

impl BodyContent for SNServiceBodyContent {}

pub type SNServiceType = NamedObjType<SNServiceDescContent, SNServiceBodyContent>;
pub type SNServiceBuilder = NamedObjectBuilder<SNServiceDescContent, SNServiceBodyContent>;
pub type SNService = NamedObjectBase<SNServiceType>;

pub trait SNServiceTrait {
    fn new(owner: ObjectId, service_type: u8, price: u64) -> SNServiceBuilder;
}

impl SNServiceTrait for NamedObjectBase<SNServiceType> {
    fn new(owner: ObjectId, service_type: u8, price: u64) -> SNServiceBuilder {
        let desc = SNServiceDescContent {
            service_type,
            price,
        };
        let body = SNServiceBodyContent {};

        SNServiceBuilder::new(desc, body).owner(owner)
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum ServiceAuthType {
    Any,
    WhiteList,
    BlackList,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNContractBodyContent {
    auth_type: ServiceAuthType,
    list: Vec<ObjectId>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNPurchase {
    pub service_id: ObjectId,
    pub start_time: u64,
    pub stop_time: u64,
    pub auth_type: ServiceAuthType,
    pub auth_list: Vec<ObjectId>,
}
