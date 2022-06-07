use std::{
    collections::HashMap, 
    convert::TryFrom
};
use serde::{Serialize, Deserialize};
use cyfs_base::*;
use crate::{
    obj_id, 
    contracts::dsg_dec_id
};

#[derive(Clone, RawEncode, RawDecode)]
pub struct DsgQueryDesc {
    hash: HashValue
}

impl DescContent for DsgQueryDesc {
    fn obj_type() -> u16 {
        obj_id::QUERY_OBJECT_TYPE
    }
    
    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}


#[derive(Serialize, Deserialize)]
pub enum DsgQuery {
    QueryContracts {
        skip: u32, 
        limit: Option<u32>
    },
    RespContracts {
        states: HashMap<ObjectId, ObjectId>
    }, 
    QueryStates {
        contracts: HashMap<ObjectId, Option<ObjectId>> 
    }, 
    RespStates {
        states: HashMap<ObjectId, ObjectId>
    }, 
} 



#[derive(RawEncode, RawDecode, Clone)]
pub struct DsgQueryBody {
    query: Vec<u8>
}


impl BodyContent for DsgQueryBody {

}

pub type DsgQueryObjectType = NamedObjType<DsgQueryDesc, DsgQueryBody>;
pub type DsgQueryObject = NamedObjectBase<DsgQueryObjectType>;


impl Into<DsgQueryObject> for DsgQuery {
    fn into(self) -> DsgQueryObject {
        let query = serde_json::to_vec(&self).unwrap();
        let hash = hash_data(query.as_slice());
        NamedObjectBuilder::new(DsgQueryDesc { hash }, DsgQueryBody { query })
            .dec_id(dsg_dec_id())
            .build()
    }
}

impl TryFrom<DsgQueryObject> for DsgQuery {
    type Error = BuckyError;
    fn try_from(obj: DsgQueryObject) -> BuckyResult<Self> {
        let query_slice = obj.body().as_ref().unwrap().content().query.as_slice();
        let hash = hash_data(query_slice);
        if obj.desc().content().hash == hash {
            let query = serde_json::from_slice(query_slice)?;
            Ok(query)
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidSignature, "hash mismatch"))
        }
    }
}