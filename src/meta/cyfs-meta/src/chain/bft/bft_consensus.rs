use cyfs_base::*;
use cyfs_base_meta::*;
use cyfs_core::CoreObjectType;

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTProtoBodyContent {
    pub data: Vec<u8>,
}

impl BodyContent for BFTProtoBodyContent {

}

#[derive(Clone, RawEncode, RawDecode)]
pub enum  BFTProtoDescContent {
    Error(BFTError),
    GetHeight,
    GetHeightResp(i64),
    GetBlock(i64),
    Tx(ObjectId),
    PrepareRequest(BFTPrepareRequest),
    PrepareResponse(BFTPrepareResponse),
    ChangeView(BFTChangeView),
    NodeSync(BFTNodeSync),
    NodeSyncResponse(BFTNodeSyncResponse),
}

impl DescContent for BFTProtoDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::MetaProto as u16
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

pub type BFTProtoDesc = NamedObjectDesc<BFTProtoDescContent>;
pub type BFTProtoType = NamedObjType<BFTProtoDescContent, BFTProtoBodyContent>;
pub type BFTProtoId = NamedObjectId<BFTProtoType>;
pub type BFTProto = NamedObjectBase<BFTProtoType>;
pub type BFTProtoBuilder = NamedObjectBuilder<BFTProtoDescContent, BFTProtoBodyContent>;

pub fn new_bft_proto(owner: ObjectId, desc: BFTProtoDescContent, body_data: Vec<u8>) -> BFTProtoBuilder {
    BFTProtoBuilder::new(desc, BFTProtoBodyContent {
        data: body_data
    }).owner(owner)
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTPrepareRequest {
    pub view: u8,
    pub speaker: u8,
    pub block_id: BlockHash,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTPrepareResponse {
    pub height: i64,
    pub view: u8,
    pub member: u8,
    pub sign: Signature,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTChangeView {
    pub height: i64,
    pub view: u8,
    pub member: u8,
    pub dest_view: u8
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTNodeSync {
    pub node_id: String,
    pub addr: String,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTNodeSyncResponse {
    pub node_id: String,
    pub addr_list: Vec<(String, String)>,
}

#[derive(Clone, RawEncode, RawDecode)]
pub struct BFTError {
    pub code: u32
}

pub struct BFTConsensus {
}

#[cfg(test)]
mod test_bft_consensus {

    #[test]
    fn test() {
    }
}
