use cyfs_base::*;
use crate::CoreObjectType;


#[derive(ProtobufEncode, ProtobufDecode, ProtobufTransformType, Clone, Debug)]
#[cyfs_protobuf_type(crate::codec::protos::NftListDescContent)]
pub struct NFTListDescContent {
    pub nft_list: Vec<FileDesc>,
}

impl ProtobufTransform<crate::codec::protos::NftListDescContent> for NFTListDescContent {
    fn transform(value: crate::codec::protos::NftListDescContent) -> BuckyResult<Self> {
        let mut nft_list = Vec::new();
        for nft in value.nft_list.iter() {
            nft_list.push(FileDesc::clone_from_slice(nft.desc.as_slice())?);
        }

        Ok(Self {
            nft_list
        })
    }
}

impl ProtobufTransform<&NFTListDescContent> for crate::codec::protos::NftListDescContent {
    fn transform(value: &NFTListDescContent) -> BuckyResult<Self> {
        let mut nft_list = Vec::new();
        for nft in value.nft_list.iter() {
            nft_list.push(crate::codec::protos::NftFileDesc {
                desc: nft.to_vec()?
            });
        }

        Ok(Self {
            nft_list
        })
    }
}

impl DescContent for NFTListDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::NFTList as u16
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

pub type NFTListDesc = NamedObjectDesc<NFTListDescContent>;
pub type NFTListType = NamedObjType<NFTListDescContent, EmptyProtobufBodyContent>;
pub type NFTListBuilder = NamedObjectBuilder<NFTListDescContent, EmptyProtobufBodyContent>;
pub type NFTList = NamedObjectBase<NFTListType>;

pub trait NFTListObject {
    fn new(owner_id: ObjectId, nft_list: Vec<FileDesc>) -> Self;
    fn nft_list(&self) -> &Vec<FileDesc>;
    fn into_nft_list(self) -> Vec<FileDesc>;
}

impl NFTListObject for NFTList {
    fn new(owner_id: ObjectId, nft_list: Vec<FileDesc>) -> Self {
        let desc = NFTListDescContent {
            nft_list
        };
        NFTListBuilder::new(desc, EmptyProtobufBodyContent::default()).owner(owner_id).build()
    }

    fn nft_list(&self) -> &Vec<FileDesc> {
        &self.desc().content().nft_list
    }

    fn into_nft_list(self) -> Vec<FileDesc> {
        self.into_desc().into_content().nft_list
    }
}
