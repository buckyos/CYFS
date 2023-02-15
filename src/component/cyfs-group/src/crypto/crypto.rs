use cyfs_base::{HashValue, ObjectId, Signature};

#[derive(Clone)]
pub struct Crypto {}

impl Crypto {
    pub fn sign(&self, hash: &HashValue) -> Signature {
        unimplemented!()
    }

    pub fn verify(&self, hash: &HashValue, sign: &Signature, object_id: &ObjectId) -> bool {
        unimplemented!()
    }
}
