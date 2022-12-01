
use crate::*;

use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub struct Nonce {
    pub nonce: u128,
    pub signs: Vec<SignData>,
    pub flags: u32,
}

impl TryFrom<protos::Nonce> for Nonce {
    type Error = BuckyError;

    fn try_from(value: protos::Nonce) -> BuckyResult<Self> {
        Ok(Self {
            nonce: ProtobufCodecHelper::decode_buf(value.get_nonce())?,
            signs: ProtobufCodecHelper::decode_buf_list(value.get_signs())?,
            flags: value.get_flags(),
        })
    }
}

impl TryFrom<&Nonce> for protos::Nonce {
    type Error = BuckyError;

    fn try_from(value: &Nonce) -> BuckyResult<Self> {
        let mut ret = protos::Nonce::new();

        ret.set_nonce(value.nonce.to_vec().unwrap());
        ret.set_signs(ProtobufCodecHelper::encode_buf_list(&value.signs)?);
        ret.set_flags(value.flags);
        
        Ok(ret)
    }
}

inner_impl_default_protobuf_raw_codec!(Nonce);

pub struct NonceHashBuilder {
    sk: PrivateKey,
}

impl NonceHashBuilder {
    pub fn build(&self, object_id: &ObjectId, nonce: u128) -> BuckyResult<HashValue> {
        use sha2::Digest;

        let mut sha256 = sha2::Sha256::new();
        sha256.input(&nonce.to_be_bytes());
        sha256.input(object_id.as_slice());
        
        let hash = sha256.clone().result();

        let sign_data = self.sk.sign_data_hash(hash.into())?;
        sha256.input(sign_data.as_slice());
        let hash = sha256.result();

        Ok(hash.into())
    }

    // hash(nonce, object_id, sign(hash(nonce, object_id)), object_id)
}

pub struct NonceVerifier<'a> {
    pk: &'a PublicKeyValue,
}

impl<'a> NonceVerifier<'a> {
    pub fn verify(&self, object_id: &ObjectId, nonce: &Nonce) -> bool {
        use sha2::Digest;

        let mut sha256 = sha2::Sha256::new();
        sha256.input(&nonce.nonce.to_be_bytes());
        sha256.input(object_id.as_slice());
        let hash = sha256.clone().result().into();

        match self.pk {
            PublicKeyValue::Single(pk) => {
                if nonce.signs.len() != 1 {
                    warn!("verify nonce but invalid signs! obj={}, nonce={}, signs={}", object_id, nonce.nonce, nonce.signs.len());
                    return false;
                }

                pk.verify_hash_data(hash, &nonce.signs[0])
            }
            PublicKeyValue::MN((threshold, pk_list)) => {
                if nonce.signs.len() < *threshold as usize {
                    return false;
                }

                if pk_list.len() < *threshold as usize {
                    return false;
                }

                // TODO signature's order must be same with the pk list order!
                let mut i = 0;
                let mut count = 0;
                for sign in &nonce.signs {
                    while i < pk_list.len() {
                        if pk_list[i].verify_hash_data(hash, sign) {
                            count += 1;
                            i += 1;
                            break;
                        }

                        i += 1;
                    }

                    if i >= pk_list.len() {
                        break;
                    }

                    if count >= *threshold as usize {
                        break;
                    }
                }

                count >= *threshold as usize
            }
        }
    }
}