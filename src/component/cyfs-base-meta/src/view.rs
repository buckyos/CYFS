use cyfs_base::*;
use super::types::*;
use super::block::{BlockHash};
use crate::{SavedMetaObject, Receipt, MetaTx, Block, NFTDesc, NFTState};
use serde::{Deserialize, Serialize};
use primitive_types::H256;

#[derive(RawEncode, RawDecode)]
pub enum ViewBlockEnum {
    Tip,
    Number(i64),
    Hash(BlockHash),
}

pub trait ViewMethod {
    type Result;
}

// 查询余额
#[derive(RawEncode, RawDecode)]
pub struct ViewBalanceMethod {
    pub account: ObjectId,
    pub ctid: Vec<CoinTokenId>
}

type ViewSignleBalanceResult = Vec<(CoinTokenId, i64)>;
type ViewUnionBalanceResult = Vec<(CoinTokenId, UnionBalance, i64)>;

#[derive(RawEncode, RawDecode)]
pub enum ViewBalanceResult {
    Single(ViewSignleBalanceResult),
    Union(ViewUnionBalanceResult)
}

impl ViewMethod for ViewBalanceMethod {
    type Result = ViewBalanceResult;
}

// 查询名字
#[derive(RawEncode, RawDecode)]
pub struct ViewNameMethod {
    pub name: String
}

type ViewNameResult = Option<(NameInfo, NameState)>;

impl ViewMethod for ViewNameMethod {
    type Result = ViewNameResult;
}

//查询desc
#[derive(RawEncode, RawDecode)]
pub struct ViewDescMethod {
    pub id: ObjectId
}

type ViewDescResult = SavedMetaObject;

impl ViewMethod for ViewDescMethod {
    type Result = ViewDescResult;
}

#[derive(RawEncode, RawDecode)]
pub struct ViewRawMethod {
    pub id: ObjectId
}

impl ViewMethod for ViewRawMethod {
    type Result = Vec<u8>;
}

#[derive(Serialize, Deserialize, RawEncode, RawDecode)]
pub struct GasPrice {
    pub low: i64,
    pub medium: i64,
    pub high: i64,
}

#[derive(Serialize, Deserialize, RawEncode, RawDecode)]
pub struct ChainStatus {
    pub version: u32,
    pub height: i64,
    pub gas_price: GasPrice,
}


#[derive(RawEncode, RawDecode)]
pub enum ViewMethodEnum {
    ViewBalance(ViewBalanceMethod),
    ViewName(ViewNameMethod),
    ViewDesc(ViewDescMethod),
    ViewRaw(ViewRawMethod),
    ViewStatus,
    ViewBlock,
    ViewTx(ObjectId),
    ViewContract(ViewContract),
    ViewBenifi(ViewBenefi),
    ViewLog(ViewLog),
    ViewNFT(ObjectId),
    ViewNFTApplyBuyList((ObjectId, u32, u8)),
    ViewNFTBidList((ObjectId, u32, u8)),
    ViewNFTLargestBuyValue(ObjectId),
}

#[derive(RawEncode, RawDecode)]
pub struct ViewRequest {
    pub block: ViewBlockEnum,
    pub method: ViewMethodEnum
}

#[derive(RawEncode, RawDecode)]
pub enum ViewResponse {
    ViewBalance(<ViewBalanceMethod as ViewMethod>::Result),
    ViewName(<ViewNameMethod as ViewMethod>::Result),
    ViewDesc(<ViewDescMethod as ViewMethod>::Result),
    ViewRaw(<ViewRawMethod as ViewMethod>::Result),
    ViewStatus(ChainStatus),
    ViewBlock(Block),
    ViewTx(TxFullInfo),
    ViewContract(ViewContractResult),
    ViewBenefi(ViewBenefiResult),
    ViewLog(ViewLogResult),
    ViewNFT((NFTDesc, String, ObjectId, NFTState)),
    ViewNFTApplyBuyList(ViewNFTBuyListResult),
    ViewNFTBidList(ViewNFTBuyListResult),
    ViewNFTLargestBuyValue(Option<(ObjectId, CoinTokenId, u64)>)
}

#[derive(RawEncode, RawDecode)]
pub struct TxFullInfo {
    pub status: u8,
    pub block_number: i64,
    pub tx: MetaTx,
    pub receipt: Option<Receipt>,
}

#[derive(RawEncode, RawDecode)]
pub struct ViewContract {
    pub address: ObjectId,
    pub data: Vec<u8>
}

#[derive(RawEncode, RawDecode)]
pub struct ViewContractResult {
    pub ret: u32,
    pub value: Vec<u8>
}

impl ViewMethod for ViewContract {
    type Result = ViewContractResult;
}

#[derive(RawEncode, RawDecode)]
pub struct ViewBenefi {
    pub address: ObjectId,
}

#[derive(RawEncode, RawDecode)]
pub struct ViewBenefiResult {
    pub address: ObjectId
}

impl ViewMethod for ViewBenefi {
    type Result = ViewBenefiResult;
}

#[derive(RawEncode, RawDecode)]
pub struct ViewLog {
    pub address: ObjectId,
    pub topics: Vec<Option<H256>>,
    pub from :i64,
    pub to :i64
}

#[derive(RawEncode, RawDecode)]
pub struct ViewLogResult {
    pub logs: Vec<(Vec<H256>, Vec<u8>)>
}

#[derive(RawEncode, RawDecode)]
pub struct NFTBuyItem {
    pub buyer_id: ObjectId,
    pub price: u64,
    pub coin_id: CoinTokenId,
}

#[derive(RawEncode, RawDecode)]
pub struct ViewNFTBuyListResult {
    pub sum: u32,
    pub list: Vec<NFTBuyItem>
}

impl ViewMethod for ViewLog {
    type Result = ViewLogResult;
}
