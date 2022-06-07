use cyfs_base::*;
use crate::CoreObjectType;
use serde::Serialize;

#[derive(ProtobufEncode, ProtobufDecode, ProtobufTransform, Clone, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::TransContextDescContent)]
pub struct TransContextDescContent {
    pub dec_id: ObjectId,
    pub context_name: String,
}

impl DescContent for TransContextDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::TransContext as u16
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(ProtobufEncode, ProtobufDecode, ProtobufTransform, Clone, Serialize)]
#[cyfs_protobuf_type(crate::codec::protos::TransContextBodyContent)]
pub struct TransContextBodyContent {
    pub ref_id: Option<ObjectId>,
    pub device_list: Vec<DeviceId>,
}

impl BodyContent for TransContextBodyContent {
    fn version(&self) -> u8 {
        0
    }

    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

pub type TransContextType = NamedObjType<TransContextDescContent, TransContextBodyContent>;
pub type TransContextBuilder = NamedObjectBuilder<TransContextDescContent, TransContextBodyContent>;
pub type TransContext = NamedObjectBase<TransContextType>;

pub trait TransContextObject {
    fn new(dec_id: ObjectId, context_name: String) -> Self;
    fn gen_context_id(dec_id: ObjectId, context_name: String) -> ObjectId;
    fn get_dec_id(&self) -> &ObjectId;
    fn get_context_name(&self) -> &str;
    fn get_ref_id(&self) -> &Option<ObjectId>;
    fn set_ref_id(&mut self, ref_id: Option<ObjectId>);
    fn get_device_list(&self) -> &Vec<DeviceId>;
    fn get_device_list_mut(&mut self) -> &mut Vec<DeviceId>;
}

impl TransContextObject for TransContext {
    fn new(dec_id: ObjectId, context_name: String) -> Self {
        let desc = TransContextDescContent { dec_id, context_name };
        let body = TransContextBodyContent { ref_id: None, device_list: vec![] };

        TransContextBuilder::new(desc, body).no_create_time().build()
    }

    fn gen_context_id(dec_id: ObjectId, context_name: String) -> ObjectId {
        let desc = TransContextDescContent { dec_id, context_name };
        NamedObjectDescBuilder::new(TransContextDescContent::obj_type(), desc)
            .option_create_time(None)
            .build()
            .calculate_id()
    }

    fn get_dec_id(&self) -> &ObjectId {
        &self.desc().content().dec_id
    }

    fn get_context_name(&self) -> &str {
        self.desc().content().context_name.as_str()
    }

    fn get_ref_id(&self) -> &Option<ObjectId> {
        &self.body().as_ref().unwrap().content().ref_id
    }

    fn set_ref_id(&mut self, ref_id: Option<ObjectId>) {
        self.body_mut().as_mut().unwrap().increase_update_time(bucky_time_now());
        self.body_mut().as_mut().unwrap().content_mut().ref_id = ref_id;
    }

    fn get_device_list(&self) -> &Vec<DeviceId> {
        &self.body().as_ref().unwrap().content().device_list
    }

    fn get_device_list_mut(&mut self) -> &mut Vec<DeviceId> {
        self.body_mut().as_mut().unwrap().increase_update_time(bucky_time_now());
        &mut self.body_mut().as_mut().unwrap().content_mut().device_list
    }
}
