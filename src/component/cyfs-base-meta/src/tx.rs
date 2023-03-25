use cyfs_base::*;
use sha2::{Digest, Sha256};
use crate::contract::*;
use crate::{MetaExtensionTx, NFTAgreeApplyTx, NFTApplyBuyTx, NFTAuctionTx, NFTBidTx, NFTBuyTx, NFTCancelApplyBuyTx, NFTCancelSellTx, NFTCreateTx, NFTCreateTx2, NFTLikeTx, NFTSellTx, NFTSellTx2, NFTSetNameTx, NFTTransTx, SNService, SPVTx};
use async_trait::async_trait;
use cyfs_core::CoreObjectType;
use generic_array::typenum::U32;
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum MetaTxBody {
    TransBalance(TransBalanceTx),
    CreateUnion(CreateUnionTx),
    DeviateUnion(DeviateUnionTx), //0x12 设置
    WithdrawFromUnion(WithdrawFromUnionTx),
    CreateDesc(CreateDescTx), //0x11 转账并创建帐号
    UpdateDesc(UpdateDescTx), //0x20
    RemoveDesc(RemoveDescTx), //0x21
    BidName(BidNameTx),       //0x30
    UpdateName(UpdateNameTx),
    TransName(TransNameTx),
    Contract(ContractTx), //0x70
    SetConfig(SetConfigTx),
    AuctionName(AuctionNameTx),
    CancelAuctionName(CancelAuctionNameTx),
    BuyBackName(BuyBackNameTx),
    BTCCoinageRecord(BTCCoinageRecordTx),
    WithdrawToOwner(WithdrawToOwner),
    CreateMinerGroup(MinerGroup),
    UpdateMinerGroup(MinerGroup),
    CreateSubChainAccount(MinerGroup),
    UpdateSubChainAccount(MinerGroup),
    SubChainWithdraw(SubChainWithdrawTx),
    WithdrawFromSubChain(WithdrawFromSubChainTx),
    SubChainCoinageRecord(SubChainCoinageRecordTx),
    Extension(MetaExtensionTx),

    // evm相关Tx
    CreateContract(CreateContractTx),
    CreateContract2(CreateContract2Tx),
    CallContract(CallContractTx),

    // 设置受益人
    SetBenefi(SetBenefiTx),

    NFTCreate(NFTCreateTx),
    NFTAuction(NFTAuctionTx),
    NFTBid(NFTBidTx),
    NFTBuy(NFTBuyTx),
    NFTSell(NFTSellTx),
    NFTApplyBuy(NFTApplyBuyTx),
    NFTCancelApplyBuyTx(NFTCancelApplyBuyTx),
    NFTAgreeApply(NFTAgreeApplyTx),
    NFTLike(NFTLikeTx),
    NFTCancelSellTx(NFTCancelSellTx),
    NFTSetNameTx(NFTSetNameTx),
    NFTCreate2(NFTCreateTx2),
    NFTSell2(NFTSellTx2),
    NFTTrans(NFTTransTx),
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct TransBalanceTx {
    pub ctid: CoinTokenId,
    //pub from: Option<ObjectId>,//from和caller可以不是同一个人
    pub to: Vec<(ObjectId, i64)>, //必须是u64,不允许from==to?,该调用完成后可能会产生no desc account.
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CreateUnionBody {
    pub account: UnionAccount,
    pub ctid: CoinTokenId,
    pub left_balance: i64,
    pub right_balance: i64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct DeviateUnionBody {
    pub ctid: CoinTokenId,
    pub seq: i64,
    pub deviation: i64, // 左边账户的改变值，左右的改变值一定是数值相等，符号相反的
    pub union: ObjectId,
}

// union_account是ffs-meta-chain 对闪电网络的的特化实现
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SignedTx<T> {
    pub body: T,
    pub signs: Vec<Signature>,
}

pub type CreateUnionTx = SignedTx<CreateUnionBody>;
impl SignedTx<CreateUnionBody> {
    pub fn new(union_account: UnionAccount, ctid: CoinTokenId) -> CreateUnionTx {
        CreateUnionTx {
            body: CreateUnionBody {
                account: union_account,
                ctid,
                left_balance: 0,
                right_balance: 0,
            },
            signs: vec![],
        }
    }

    pub fn set_recharge_amount(&mut self, account: &ObjectId, amount: i64) -> BuckyResult<()> {
        if self.body.account.desc().content().left() == account {
            self.body.left_balance = amount;
        } else if self.body.account.desc().content().right() == account {
            self.body.right_balance = amount;
        } else {
            return Err(BuckyError::new(BuckyErrorCode::Failed, "Failed"));
        }
        Ok(())
    }
}

pub type DeviateUnionTx = SignedTx<DeviateUnionBody>;
impl SignedTx<DeviateUnionBody> {
    pub fn new(union: ObjectId, ctid: CoinTokenId, seq: i64, deviation: i64) -> DeviateUnionTx {
        DeviateUnionTx {
            body: DeviateUnionBody {
                ctid,
                seq,
                deviation,
                union,
            },
            signs: vec![],
        }
    }
}

impl<T: RawEncode> SignedTx<T> {
    pub fn sign(&mut self, id: ObjectLink, secret: PrivateKey) -> BuckyResult<()> {
        let mut i = 0;
        let mut find = false;
        for sign in &self.signs {
            if let SignatureSource::Object(sign_obj_link) = sign.sign_source() {
                if *sign_obj_link == id {
                    find = true;
                    break;
                }
            }
            i += 1;
        }
        if find {
            self.signs.remove(i);
        }

        let signer = RsaCPUObjectSigner::new(secret.public(), secret);
        let data = self.body.to_vec().map_err(|e| {
            log::error!("UnionAccountTx<T>::sign/body.to_vec error:{}", e);
            e
        })?;

        let hash = hash_data(data.as_slice());
        let signature =
            async_std::task::block_on(signer.sign(hash.as_slice(), &SignatureSource::Object(id)))
                .map_err(|e| {
                log::error!("UnionAccountTx<T>::sign/sign error:{}", e);
                e
            })?;

        self.signs.push(signature);
        Ok(())
    }

    pub async fn async_sign(&mut self, id: ObjectLink, secret: PrivateKey) -> BuckyResult<()> {
        let mut i = 0;
        let mut find = false;
        for sign in &self.signs {
            if let SignatureSource::Object(sign_obj_link) = sign.sign_source() {
                if *sign_obj_link == id {
                    find = true;
                    break;
                }
            }
            i += 1;
        }
        if find {
            self.signs.remove(i);
        }

        let signer = RsaCPUObjectSigner::new(secret.public(), secret);
        let data = self.body.to_vec().map_err(|e| {
            log::error!("UnionAccountTx<T>::async_sign/body.to_vec error:{}", e);
            e
        })?;

        let hash = hash_data(data.as_slice());
        let signature = signer
            .sign(hash.as_slice(), &SignatureSource::Object(id))
            .await
            .map_err(|e| {
                log::error!("UnionAccountTx<T>::async_sign/sign error:{}", e);
                e
            })?;

        self.signs.push(signature);
        Ok(())
    }

    pub fn verify(&self, key_list: Vec<(ObjectId, PublicKey)>) -> BuckyResult<bool> {
        let data = self.body.to_vec().map_err(|e| {
            log::error!("UnionAccountTx<T>::verify/body.to_vec error:{}", e);
            e
        })?;

        let hash = hash_data(data.as_slice());
        for (id, public_key) in key_list {
            let mut verify = false;
            for sign in &self.signs {
                if let SignatureSource::Object(sign_obj_link) = sign.sign_source() {
                    if sign_obj_link.obj_id == id {
                        let verifier = RsaCPUObjectVerifier::new(public_key.clone());
                        let verify_ret =
                            async_std::task::block_on(verifier.verify(hash.as_slice(), sign));
                        if verify_ret {
                            verify = verify_ret;
                            break;
                        }
                    }
                }
            }
            if !verify {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct WithdrawToOwner {
    pub ctid: CoinTokenId,
    pub id: ObjectId,
    pub value: i64,
}

// union_account是ffs-meta-chain 对闪电网络的的特化实现
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct WithdrawFromUnionTx {
    pub ctid: CoinTokenId,
    pub union: ObjectId,
    pub value: i64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct Data {
    pub id: ObjectId,
    pub data: Vec<u8>,
}

// impl RawEncode for Data {
//     fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
//         Ok(u16::raw_bytes().unwrap() + self.data.len() + self.id.raw_measure(purpose).map_err(|e|{
//             log::error!("TxData::raw_measure/id error:{}", e);
//             e
//         })?)
//     }
//
//     fn raw_encode<'a>(&self, buf: &'a mut [u8], _purpose: &Option<RawEncodePurpose>) -> BuckyResult<&'a mut [u8]> {
//         if self.data.len() > u16::MAX as usize{
//             return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, format!("raw encode able vector length more than {}", u16::MAX)));
//         }
//
//         let buf = self.id.raw_encode(buf, None).map_err(|e|{
//             log::error!("TxData::raw_encode/id error:{}", e);
//             e
//         })?;
//
//         let buf = (self.data.len() as u16).raw_encode(buf, &None).map_err(|e|{
//             log::error!("TxData::raw_encode/data len error:{}", e);
//             e
//         })?;
//
//         if buf.len() < self.data.len() {
//             return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "not enough buffer"));
//         }
//
//         buf[0..self.data.len()].copy_from_slice(self.data.as_slice());
//
//         // buf.copy_from_slice(self.data.as_slice());
//
//         Ok(&mut buf[self.data.len()..])
//     }
// }
//
// impl<'de> RawDecode<'de> for Data {
//     fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
//         let (id, buf) = ObjectId::raw_decode(buf).map_err(|e|{
//             log::error!("TxData::raw_decode/id error:{}", e);
//             e
//         })?;
//
//         let (len, buf) = u16::raw_decode(buf).map_err(|e|{
//             log::error!("TxData::raw_decode/data len error:{}", e);
//             e
//         })?;
//
//         if buf.len() < len as usize {
//             return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, "not enough buffer"));
//         }
//         let data = &buf[..len as usize];
//
//         Ok((Data {
//             id,
//             data: Vec::from(data),
//         }, &buf[(len as usize)..]))
//     }
// }

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum SavedMetaObject {
    Device(Device),             //BDT中定义
    People(People),             //BDT中定义
    UnionAccount(UnionAccount), //两人公有账号，用于闪电网络
    Group(SimpleGroup),         //M*N Group,最常见的Group BDT中定义
    File(File),
    Data(Data),
    Org(Org),
    MinerGroup(MinerGroup),
    SNService(SNService),
    Contract(Contract),
}

impl SavedMetaObject {
    pub fn hash(&self) -> BuckyResult<HashValue> {
        let data = self.to_vec()?;
        let mut hasher = Sha256::new();
        hasher.input(data);
        Ok(HashValue::from(hasher.result()))
    }

    pub fn id(&self) -> ObjectId {
        match self {
            SavedMetaObject::Device(o) => {o.desc().calculate_id()}
            SavedMetaObject::People(o) => {o.desc().calculate_id()}
            SavedMetaObject::UnionAccount(o) => {o.desc().calculate_id()}
            SavedMetaObject::Group(o) => {o.desc().calculate_id()}
            SavedMetaObject::File(o) => {o.desc().calculate_id()}
            SavedMetaObject::Data(o) => {o.id.clone()}
            SavedMetaObject::Org(o) => {o.desc().calculate_id()}
            SavedMetaObject::MinerGroup(o) => {o.desc().calculate_id()}
            SavedMetaObject::SNService(o) => {o.desc().calculate_id()}
            SavedMetaObject::Contract(o) => {o.desc().calculate_id()}
        }
    }
}

impl TryFrom<SavedMetaObject> for AnyNamedObject {
    type Error = BuckyError;

    fn try_from(value: SavedMetaObject) -> Result<Self, Self::Error> {
        match value {
            SavedMetaObject::Data(v) => Ok(AnyNamedObject::clone_from_slice(&v.data)?),
            SavedMetaObject::MinerGroup(v) => Ok(AnyNamedObject::clone_from_slice(&v.to_vec()?)?),
            SavedMetaObject::SNService(v) => Ok(AnyNamedObject::clone_from_slice(&v.to_vec()?)?),
            v @ _ => Ok(AnyNamedObject::Standard(StandardObject::try_from(v)?)),
        }
    }
}

impl TryFrom<SavedMetaObject> for StandardObject {
    type Error = BuckyError;
    fn try_from(object: SavedMetaObject) -> Result<Self, Self::Error> {
        match object {
            SavedMetaObject::Device(v) => Ok(Self::Device(v)),
            SavedMetaObject::People(v) => Ok(Self::People(v)),
            SavedMetaObject::UnionAccount(v) => Ok(Self::UnionAccount(v)),
            SavedMetaObject::Group(v) => Ok(Self::SimpleGroup(v)),
            SavedMetaObject::File(v) => Ok(Self::File(v)),
            SavedMetaObject::Org(v) => Ok(Self::Org(v)),
            SavedMetaObject::Contract(v) => Ok(Self::Contract(v)),
            _ => Err(BuckyError::from(BuckyErrorCode::NotSupport)),
        }
    }
}

impl TryFrom<StandardObject> for SavedMetaObject {
    type Error = BuckyError;

    fn try_from(object: StandardObject) -> Result<Self, Self::Error> {
        let ret = match object {
            StandardObject::Device(v) => Self::Device(v),
            StandardObject::People(v) => Self::People(v),
            StandardObject::UnionAccount(v) => Self::UnionAccount(v),
            StandardObject::SimpleGroup(v) => SavedMetaObject::Group(v),
            StandardObject::File(v) => Self::File(v),
            StandardObject::Org(v) => Self::Org(v),
            StandardObject::Contract(v) => Self::Contract(v),
            _ => {
                return Err(BuckyError::from(BuckyErrorCode::NotSupport));
            }
        };

        Ok(ret)
    }
}

//租用Meta-Chain的空间来创建对象，操作已创建对象不需要再在Tx中带Desc
//有一些Object的操作，会强制要求对象先创建。
//租用需要持续扣费，这些扣费操作需要作为类似CoinBase的操作保留在区块里么？
//TODO:考虑租用系统如何实现
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CreateDescTx {
    pub coin_id: u8,
    pub from: Option<ObjectId>, //from和op可以不是同一个人
    //pub to: ObjectId,//TODO:可以从desc中算出来，不用带
    pub value: i64,
    pub desc_hash: HashValue,
    pub price: u32, //租金，租金太低可能不会被meta-chain接受
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MetaPrice {
    pub coin_id: u8,
    pub price: u32,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct UpdateDescTx {
    //pub id : ObjectId,//TODO:可以从desc中算出来
    pub write_flag: u8, //为了减少手续费，可以指定更新方式
    pub price: Option<MetaPrice>,
    pub desc_hash: HashValue,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct RemoveDescTx {
    pub id: ObjectId,
}

//名字体系是ffs-meta的核心功能，目的是取代dns
//买了一级的名字，那么在2级名字没有出售的情况下，可以在DescBody中定义2级名字而不需要购买（类似DNS)
//名字的价格体系和Id不完全相同，但其核心依旧是租用系统
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct BidNameTx {
    pub name: String,
    pub owner: Option<ObjectId>, //opid可以和owner不同
    pub name_price: u64,         //name的购买价格，coin_id有meta指定
    pub price: u32,              //租金，coin_id meta指定
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct UpdateNameTx {
    pub name: String,
    pub info: NameInfo,
    pub write_flag: u8,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct AuctionNameTx {
    pub name: String,
    pub price: u64, //起始拍卖价
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct CancelAuctionNameTx {
    pub name: String,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct BuyBackNameTx {
    pub name: String,
}

//TODO： 下面两个先不做 通过该TX可以创建2级域名
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct TransNameTx {
    pub sub_name: Option<String>,
    pub new_owner: ObjectId,
}

//---------- 合约的先不做 ---------
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct InstanceContractTx {
    pub contract_id: ObjectId,
    pub template_parms: Vec<u8>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ContractTx {
    pub instance_id: ObjectId, //instance id
    pub func_name: String,
    pub parm_body: Vec<u8>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum TxLog {
    ContractLog(ContractLog),
    ExtensionTx(Vec<u8>),
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ContractLog {
    pub address: ObjectId,
    pub topics: Vec<GenericArray<u8, U32>>,
    pub data: Vec<u8>,
}

impl From<crate::evm_def::Log> for TxLog {
    fn from(log: crate::evm_def::Log) -> Self {
        TxLog::ContractLog(ContractLog {
            address: log.address,
            topics: log
                .topics
                .iter()
                .map(|topic| GenericArray::clone_from_slice(topic.as_bytes()))
                .collect(),
            data: log.data,
        })
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ReceiptV1 {
    pub result: u32,
    pub fee_used: u32,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ReceiptV2 {
    pub result: u32,
    pub fee_used: u32,
    pub logs: Vec<TxLog>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct Receipt {
    pub result: u32,
    pub fee_used: u32,
    pub logs: Vec<TxLog>,
    pub address: Option<ObjectId>,
    pub return_value: Option<Vec<u8>>,
}

impl From<&ReceiptV1> for Receipt {
    fn from(r: &ReceiptV1) -> Self {
        Self {
            result: r.result,
            fee_used: r.fee_used,
            logs: vec![],
            address: None,
            return_value: None,
        }
    }
}

impl From<ReceiptV1> for Receipt {
    fn from(r: ReceiptV1) -> Self {
        (&r).into()
    }
}

impl From<&ReceiptV2> for Receipt {
    fn from(r: &ReceiptV2) -> Self {
        Self {
            result: r.result,
            fee_used: r.fee_used,
            logs: r.logs.clone(),
            address: None,
            return_value: None,
        }
    }
}

impl Receipt {
    pub fn new(result: u32, fee_used: u32) -> Self {
        Self {
            result,
            fee_used,
            logs: vec![],
            address: None,
            return_value: None,
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SetConfigTx {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, RawEncode, RawDecode)]
#[allow(non_snake_case)]
pub struct BTCTxRecord {
    pub txid: String,
    pub blockHash: String,
    pub blockNumber: u64,
    pub confirmed: u64,
    pub received: u64,
    pub exodusAddress: String,
    pub btcValue: u64,
    pub version: u32,
    pub propertyID: u32,
    pub op: u32,
    pub address: String,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct BTCCoinageRecordTx {
    pub height: u64,
    pub list: Vec<BTCTxRecord>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MinerGroupDescContent {}

impl DescContent for MinerGroupDescContent {
    fn obj_type() -> u16 {
        CoreObjectType::MetaMinerGroup as u16
    }

    type OwnerType = SubDescNone;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MinerGroupBodyContent {
    pub members: Vec<DeviceDesc>,
}

impl BodyContent for MinerGroupBodyContent {}

pub type MinerGroupType = NamedObjType<MinerGroupDescContent, MinerGroupBodyContent>;
pub type MinerGroupBuilder = NamedObjectBuilder<MinerGroupDescContent, MinerGroupBodyContent>;
pub type MinerGroup = NamedObjectBase<MinerGroupType>;

pub trait MinerGroupTrait {
    fn new(members: Vec<DeviceDesc>) -> MinerGroupBuilder;
    fn members(&self) -> &Vec<DeviceDesc>;
    fn members_mut(&mut self) -> &mut Vec<DeviceDesc>;
    fn has_member(&self, id: &ObjectId) -> bool;
}

impl MinerGroupTrait for NamedObjectBase<MinerGroupType> {
    fn new(members: Vec<DeviceDesc>) -> MinerGroupBuilder {
        let desc = MinerGroupDescContent {};
        let body = MinerGroupBodyContent { members };

        MinerGroupBuilder::new(desc, body)
    }

    fn members(&self) -> &Vec<DeviceDesc> {
        &self.body().as_ref().unwrap().content().members
    }

    fn members_mut(&mut self) -> &mut Vec<DeviceDesc> {
        &mut self.body_mut().as_mut().unwrap().content_mut().members
    }

    fn has_member(&self, id: &ObjectId) -> bool {
        let members = &self.body().as_ref().unwrap().content().members;
        for member in members {
            if &member.calculate_id() == id {
                return true;
            }
        }
        return false;
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct MinerDesc {}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SubChainWithdrawTx {
    pub subchain_id: ObjectId,
    pub withdraw_tx: Vec<u8>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct WithdrawFromSubChainTx {
    pub coin_id: CoinTokenId,
    pub value: i64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct SubChainCoinageRecordTx {
    pub height: i64,
    pub list: Vec<SPVTx>,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct FlowServiceDescContent {}

impl DescContent for FlowServiceDescContent {
    fn obj_type() -> u16 {
        0_u16
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct FlowServiceBodyContent {
    price: i64,
}

impl BodyContent for FlowServiceBodyContent {}

pub type FlowServiceType = NamedObjType<FlowServiceDescContent, FlowServiceBodyContent>;
pub type FlowServiceBuilder = NamedObjectBuilder<FlowServiceDescContent, FlowServiceBodyContent>;
pub type FlowService = NamedObjectBase<FlowServiceType>;

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum FlowServiceTx {
    Create(FlowService),
    Purchase(u32),
    Settle,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum SNServiceTx {
    Publish(SNService),
    Remove(ObjectId),
    Purchase(Contract),
    Settle(ProofOfService),
}

pub type MetaTxDescContent = TxDescContent<TypeBuffer<Vec<MetaTxBody>>>;

pub type MetaTxDesc = NamedObjectDesc<MetaTxDescContent>;
pub type MetaTxType = NamedObjType<MetaTxDescContent, TxBodyContent>;
pub type MetaTxId = NamedObjectId<MetaTxType>;
pub type MetaTx = NamedObjectBase<MetaTxType>;
pub type MetaTxBuilder = NamedObjectBuilder<MetaTxDescContent, TxBodyContent>;

pub trait MetaTxDescTrait {
    fn tx_id(&self) -> TxId;
}

impl MetaTxDescTrait for NamedObjectDesc<MetaTxDescContent> {
    fn tx_id(&self) -> TxId {
        TxId::try_from(self.calculate_id()).unwrap()
    }
}

#[async_trait]
pub trait MetaTxTrait {
    fn new(
        nonce: i64,
        caller: TxCaller,
        gas_coin_id: u8,
        gas_price: u16,
        max_fee: u32,
        condition: Option<TxCondition>,
        body: MetaTxBody,
        data: Vec<u8>,
    ) -> MetaTxBuilder;
    fn new_multi_body(
        nonce: i64,
        caller: TxCaller,
        gas_coin_id: u8,
        gas_price: u16,
        max_fee: u32,
        condition: Option<TxCondition>,
        bodys: Vec<MetaTxBody>,
        data: Vec<u8>,
    ) -> MetaTxBuilder;
    fn verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool>;
    async fn async_verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool>;
    fn sign(&mut self, secret: PrivateKey) -> BuckyResult<()>;
    async fn async_sign(&mut self, secret: PrivateKey) -> BuckyResult<()>;
}
/// 交易对象
/// ====
/// new 传入 TxBody，注意 TxBody不是 Tx.body()，这个地方的命名有点冲突
/// new_multi_body 传入 Vec<TxBody>
///
#[async_trait]
impl MetaTxTrait for NamedObjectBase<MetaTxType> {
    fn new(
        nonce: i64,
        caller: TxCaller,
        gas_coin_id: u8,
        gas_price: u16,
        max_fee: u32,
        condition: Option<TxCondition>,
        body: MetaTxBody,
        data: Vec<u8>,
    ) -> MetaTxBuilder {
        let desc_content = MetaTxDescContent {
            nonce,
            caller,
            gas_coin_id,
            gas_price,
            max_fee,
            condition,
            body: TypeBuffer::from(vec![body]),
        };

        let body_content = TxBodyContent::new(data);

        MetaTxBuilder::new(desc_content, body_content)
    }

    fn new_multi_body(
        nonce: i64,
        caller: TxCaller,
        gas_coin_id: u8,
        gas_price: u16,
        max_fee: u32,
        condition: Option<TxCondition>,
        bodys: Vec<MetaTxBody>,
        data: Vec<u8>,
    ) -> MetaTxBuilder {
        let desc_content = MetaTxDescContent {
            nonce,
            caller,
            gas_coin_id,
            gas_price,
            max_fee,
            condition,
            body: TypeBuffer::from(bodys),
        };

        let body_content = TxBodyContent::new(data);

        MetaTxBuilder::new(desc_content, body_content)
    }

    fn verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool> {
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

    async fn async_verify_signature(&self, public_key: PublicKey) -> BuckyResult<bool> {
        if self.desc().content().caller.is_miner() {
            return Ok(true);
        }
        if self.desc().content().caller.is_fake() {
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

    fn sign(&mut self, secret: PrivateKey) -> BuckyResult<()> {
        let signer = RsaCPUObjectSigner::new(secret.public(), secret);
        async_std::task::block_on(sign_and_set_named_object(
            &signer,
            self,
            &SignatureSource::RefIndex(0),
        ))
    }

    async fn async_sign(&mut self, secret: PrivateKey) -> BuckyResult<()> {
        let signer = RsaCPUObjectSigner::new(secret.public(), secret);
        sign_and_set_named_object(&signer, self, &SignatureSource::RefIndex(0)).await
    }
}

#[cfg(test)]
mod tx_test {
    use crate::*;
    use cyfs_base::*;

    #[test]
    fn test() {
        let body = TransBalanceTx {
            ctid: CoinTokenId::Coin(0),
            to: vec![(ObjectId::default(), 2), (ObjectId::default(), 3)],
        };

        let meta_tx = MetaTx::new(
            1,
            TxCaller::Miner(ObjectId::default()),
            0,
            0,
            0,
            None,
            MetaTxBody::TransBalance(body),
            Vec::new(),
        )
        .build();

        let ret = meta_tx.to_vec();
        assert!(ret.is_ok());

        let ret = Tx::clone_from_slice(ret.unwrap().as_slice());
        assert!(ret.is_ok());

        let ret = ret.unwrap().to_vec();
        assert!(ret.is_ok());

        let ret = MetaTx::clone_from_slice(ret.unwrap().as_slice());
        assert!(ret.is_ok());

        let meta_tx = ret.unwrap();
        let body = meta_tx.desc().content().body.get_obj().get(0).unwrap();
        if let MetaTxBody::TransBalance(trans_tx) = body {
            assert_eq!(trans_tx.to.len(), 2);
            assert_eq!(trans_tx.to.get(0).unwrap().1, 2);
            assert_eq!(trans_tx.to.get(1).unwrap().1, 3);
        } else {
            assert!(false);
        }
    }

    const OLD_LIST: &'static [&'static str] = &[
        "010002500e0000000000010030818902818100e0252144cac6aa8493f252c1c7d288afd9d01f04430a24f19bbd1f0fec428278b149f3b748e26a532c7e238dcdde6fb60d3820727f53b7ae090ce1bb04f637d43aea4551043a06535ded73e6a7de845e6a6187cfcd4def56b841fd098afc0671f659bfbabd1fbceb268b6fa0f47b8c7e3cb698a2d6ba120e54b6df9064c889ed0203010001000000000000000000000000000000000000000000000000000000002f3b2f6e3acd9000013f0a2045c40d30000cd65e863aa69f59d818f2090e3fa3b3d646dadc87568f773da50c120fe7bab3e696afe8b59be58d9ae4bcaf220a7374616e64616c6f6e650100ff002f3b2f6e3ad56000c661eeddb115b2b1cc8f6a0abe871b83c3108379c766303457676e1beca4836220767e72cac8fa53b3addff7686a604ee5537b4593d7ef363dcfd827dace32a51fb46c527330d6cb1a2bf37a4fcc4d6bae9ed5acf0289c7f6fa3e957c4a11362bf237746a253edd574c59acf85705d36747dd83ac65acd0995c201e76be7db5f0100ff002f3b2f6e3e09b000bb6a8d783a3be84580323e7e70b7aeb0c277c60c09705218fe77eb5a527df5cfdfe738d05c0661c9e06f59c883d7b314d4709d56675cecdde81bbb72dc692c5413c9a39f81aaf7fd928319f7bd4183132ab3c383b6e39a924a87ba1608133cbd2a6bdfeb2e613752971d24c944ed666ed5d50c9a77177ab4077143c5354f90c2",
    ];

    #[test]
    fn test_people_codec() {

        for code in OLD_LIST {
            let code = hex::decode(code).unwrap();
            let ret = SavedMetaObject::clone_from_slice(code.as_slice()).unwrap();

            let hash = ret.hash().unwrap();
            println!("desc hash: {}", hash);
        }
    }
}
