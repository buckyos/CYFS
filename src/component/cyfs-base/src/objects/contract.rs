use crate::codec as cyfs_base;
use crate::*;

use generic_array::typenum::U32;
use generic_array::GenericArray;
use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ERC20 {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNContract {
    pub service_id: ObjectId,
    pub account: UnionAccount,
    pub start_time: u64,
    pub stop_time: u64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum ServiceAuthType {
    Any,
    WhiteList,
    BlackList,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SNContractBody {
    auth_type: ServiceAuthType,
    list: Vec<ObjectId>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum ContractTypeCode {
    DSG,
    Solidity,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ContractDescContent<T> {
    pub contract_type: ContractTypeCode,
    pub data: T,
}

impl<T> Deref for ContractDescContent<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ContractDescContent<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ContractData {
    pub data: Vec<u8>,
}

impl Deref for ContractData {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DescContent for ContractDescContent<T> {
    fn obj_type() -> u16 {
        ObjectTypeCode::Contract.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = Option<ObjectId>;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug)]
pub struct ContractBodyContent<T> {
    pub data: T,
}

impl TryFrom<protos::ContractBodyContent> for ContractBodyContent<ContractData> {
    type Error = BuckyError;

    fn try_from(mut value: protos::ContractBodyContent) -> BuckyResult<Self> {
        Ok(Self {
            data: ContractData {
                data: value.take_data(),
            },
        })
    }
}

impl TryFrom<&ContractBodyContent<ContractData>> for protos::ContractBodyContent {
    type Error = BuckyError;

    fn try_from(value: &ContractBodyContent<ContractData>) -> BuckyResult<Self> {
        let mut ret = protos::ContractBodyContent::new();
        ret.set_data(value.data.data.clone());

        Ok(ret)
    }
}

impl<T> BodyContent for ContractBodyContent<T> {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

type ContractBodyContentContractData = ContractBodyContent<ContractData>;
inner_impl_default_protobuf_raw_codec!(
    ContractBodyContentContractData,
    protos::ContractBodyContent
);

type ContractType =
    NamedObjType<ContractDescContent<ContractData>, ContractBodyContent<ContractData>>;
pub type ContractBuilder =
    NamedObjectBuilder<ContractDescContent<ContractData>, ContractBodyContent<ContractData>>;

pub type ContractDesc = NamedObjectDesc<ContractDescContent<ContractData>>;
pub type ContractId = NamedObjectId<ContractType>;
pub type Contract = NamedObjectBase<ContractType>;

impl ContractDesc {
    pub fn contract_id(&self) -> ContractId {
        ContractId::try_from(self.calculate_id()).unwrap()
    }
}

impl ContractId {
    pub fn from_hash_256(hash: GenericArray<u8, U32>) -> Self {
        let mut id = Self::default();
        id.object_id_mut().as_mut_slice()[1..].copy_from_slice(&hash[1..]);
        id
    }
}

impl NamedObjectBase<ContractType> {
    pub fn new(
        owner: ObjectId,
        author: ObjectId,
        desc: ContractDescContent<ContractData>,
        body: ContractBodyContent<ContractData>,
    ) -> ContractBuilder {
        ContractBuilder::new(desc, body).owner(owner).author(author)
    }
}

#[cfg(test)]
mod test {
    use crate::{Contract, RawConvertTo, RawFrom};
    use crate::{
        ContractBodyContent, ContractData, ContractDescContent, ContractTypeCode, ObjectId,
    };

    #[test]
    fn contract() {
        let desc = ContractDescContent {
            contract_type: ContractTypeCode::DSG,
            data: ContractData { data: Vec::new() },
        };

        let body = ContractBodyContent {
            data: ContractData { data: Vec::new() },
        };

        let object = Contract::new(ObjectId::default(), ObjectId::default(), desc, body).build();

        // let p = Path::new("f:\\temp\\contract.obj");
        // if p.parent().unwrap().exists() {
        //     object.clone().encode_to_file(p, false);
        // }

        let buf = object.to_vec().unwrap();
        let _obj = Contract::clone_from_slice(&buf).unwrap();
    }
}
