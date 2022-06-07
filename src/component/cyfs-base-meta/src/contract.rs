use cyfs_base::{ObjectId};
use cyfs_base::{RawEncode, RawDecode, RawEncodePurpose, BuckyResult};
use generic_array::GenericArray;
use generic_array::typenum::{U32};

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CreateContractTx {
    pub value: u64,
    pub init_data: Vec<u8>
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CreateContract2Tx {
    pub value: u64,
    pub init_data: Vec<u8>,
    pub salt: GenericArray<u8, U32>
}

impl CreateContract2Tx {
    pub fn new(value: u64, init_data: Vec<u8>, salt: [u8;32]) -> Self {
        Self {
            value,
            init_data,
            salt: GenericArray::clone_from_slice(salt.as_ref())
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CallContractTx {
    pub address: ObjectId,
    pub value: u64,
    pub data: Vec<u8>
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SetBenefiTx {
    pub address: ObjectId,
    pub to: ObjectId
}