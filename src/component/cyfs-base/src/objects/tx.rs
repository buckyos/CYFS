use crate::codec as cyfs_base;
use crate::*;

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum TxCondition {
    //时间
//BTC 交易确认
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, RawEncode, RawDecode, Eq, PartialEq)]
pub enum CoinTokenId {
    Coin(u8),
    Token(ObjectId),
}

impl ProtobufTransform<CoinTokenId> for Vec<u8> {
    fn transform(value: CoinTokenId) -> BuckyResult<Self> {
        value.to_vec()
    }
}

impl ProtobufTransform<&CoinTokenId> for Vec<u8> {
    fn transform(value: &CoinTokenId) -> BuckyResult<Self> {
        value.to_vec()
    }
}

impl ProtobufTransform<Vec<u8>> for CoinTokenId {
    fn transform(value: Vec<u8>) -> BuckyResult<Self> {
        CoinTokenId::clone_from_slice(value.as_slice())
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum TxCaller {
    People(PeopleDesc),
    Device(DeviceDesc),
    Group(SimpleGroupDesc),
    Union(UnionAccountDesc),
    Miner(ObjectId),
    Id(ObjectId),
}

impl TryFrom<&StandardObject> for TxCaller {
    type Error = BuckyError;
    fn try_from(obj: &StandardObject) -> Result<Self, Self::Error> {
        match obj {
            StandardObject::People(desc) => Ok(Self::People(desc.desc().clone())),
            StandardObject::Device(desc) => Ok(Self::Device(desc.desc().clone())),
            StandardObject::SimpleGroup(desc) => Ok(Self::Group(desc.desc().clone())),
            StandardObject::UnionAccount(desc) => Ok(Self::Union(desc.desc().clone())),
            _ => Err(BuckyError::new(BuckyErrorCode::Failed, "Failed")),
        }
    }
}

impl TxCaller {
    pub fn id(&self) -> BuckyResult<ObjectId> {
        let id = match self {
            Self::People(desc) => desc.calculate_id(),
            Self::Device(desc) => desc.calculate_id(),
            Self::Group(desc) => desc.calculate_id(),
            Self::Union(desc) => desc.calculate_id(),
            Self::Miner(id) => id.clone(),
            Self::Id(id) => id.clone(),
        };
        Ok(id)
    }

    pub fn get_public_key(&self) -> BuckyResult<&PublicKey> {
        match self {
            Self::People(desc) => Ok(desc.public_key()),
            Self::Device(desc) => Ok(desc.public_key()),
            Self::Group(_desc) => Err(BuckyError::new(BuckyErrorCode::Failed, "Failed")),
            Self::Union(_desc) => Err(BuckyError::new(BuckyErrorCode::Failed, "Failed")),
            Self::Miner(_) => Err(BuckyError::new(BuckyErrorCode::Failed, "Failed")),
            Self::Id(id) => {
                if id.is_default() {
                    Ok(&PublicKey::Invalid)
                } else {
                    Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"))
                }
            },
        }
    }

    pub fn is_miner(&self) -> bool {
        match self {
            Self::Miner(_) => true,
            _ => false,
        }
    }

    pub fn is_fake(&self) -> bool {
        match self {
            Self::Id(id) => {
                id.is_default()
            },
            _ => false
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct TxDescContent<T> {
    pub nonce: i64,
    pub caller: TxCaller,
    pub gas_coin_id: u8, //用哪种coin来支付手续费
    pub gas_price: u16,  //
    pub max_fee: u32,
    pub condition: Option<TxCondition>, //Tx的生效条件，用于创建上链后不立刻生效的TX,
    pub body: T,
    // pub tx_body_buf: Vec<u8>,
    // pub bodys: Vec<TxBody> //定义参考旧代码
}

impl<T> DescContent for TxDescContent<T> {
    fn obj_type() -> u16 {
        ObjectTypeCode::Tx.into()
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug)]
pub struct TxBodyContent {
    pub data: Vec<u8>,
}

impl BodyContent for TxBodyContent {
    fn format(&self) -> u8 {
        OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF
    }
}

impl TxBodyContent {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

// body使用protobuf编解码
impl TryFrom<protos::TxBodyContent> for TxBodyContent {
    type Error = BuckyError;

    fn try_from(mut value: protos::TxBodyContent) -> BuckyResult<Self> {
        Ok(Self {
            data: value.take_data(),
        })
    }
}

impl TryFrom<&TxBodyContent> for protos::TxBodyContent {
    type Error = BuckyError;

    fn try_from(value: &TxBodyContent) -> BuckyResult<Self> {
        let mut ret = Self::new();

        ret.set_data(value.data.to_owned());

        Ok(ret)
    }
}

crate::inner_impl_default_protobuf_raw_codec!(TxBodyContent);

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct TxBody {
    body: Vec<u8>,
}

pub type TxDesc = NamedObjectDesc<TxDescContent<TxBody>>;
pub type TxType = NamedObjType<TxDescContent<TxBody>, TxBodyContent>;
pub type TxId = NamedObjectId<TxType>;
pub type Tx = NamedObjectBase<TxType>;
pub type TxBuilder = NamedObjectBuilder<TxDescContent<TxBody>, TxBodyContent>;

impl NamedObjectDesc<TxDescContent<Vec<u8>>> {
    pub fn tx_id(&self) -> TxId {
        TxId::try_from(self.calculate_id()).unwrap()
    }
}

/// 交易对象
/// ====
/// new 传入 TxBody，注意 TxBody不是 Tx.body()，这个地方的命名有点冲突
/// new_multi_body 传入 Vec<TxBody>
///
impl NamedObjectBase<TxType> {
    pub fn new(
        nonce: i64,
        caller: TxCaller,
        gas_coin_id: u8,
        gas_price: u16,
        max_fee: u32,
        condition: Option<TxCondition>,
        body: Vec<u8>,
        data: Vec<u8>,
    ) -> TxBuilder {
        let desc_content = TxDescContent {
            nonce,
            caller,
            gas_coin_id,
            gas_price,
            max_fee,
            condition,
            body: TxBody { body },
        };

        let body_content = TxBodyContent { data };

        TxBuilder::new(desc_content, body_content)
    }

    pub fn verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool> {
        if self.desc().content().caller.is_miner() {
            return Ok(true);
        }
        let desc_signs = self.signs().desc_signs();
        if desc_signs.is_none() {
            return Ok(false);
        }

        let signs = desc_signs.as_ref().unwrap();
        if signs.len() == 0 {
            return Ok(false);
        }

        let sign = signs.get(0).unwrap();
        let verifier = RsaCPUObjectVerifier::new(public_key);

        async_std::task::block_on(verify_object_desc_sign(&verifier, self, sign))
    }

    pub async fn async_verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool> {
        if self.desc().content().caller.is_miner() {
            return Ok(true);
        }
        let desc_signs = self.signs().desc_signs();
        if desc_signs.is_none() {
            return Ok(false);
        }

        let signs = desc_signs.as_ref().unwrap();
        if signs.len() == 0 {
            return Ok(false);
        }

        let sign = signs.get(0).unwrap();
        let verifier = RsaCPUObjectVerifier::new(public_key);
        verify_object_desc_sign(&verifier, self, sign).await
    }

    pub fn sign(&mut self, secret: &PrivateKey) -> BuckyResult<()> {
        let signer = RsaCPUObjectSigner::new(secret.public(), secret.clone());
        async_std::task::block_on(sign_and_set_named_object(
            &signer,
            self,
            &SignatureSource::RefIndex(0),
        ))
    }
}
