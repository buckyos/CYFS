use serde::{Serialize, Deserialize};
use cyfs_base::BuckyError;
use cyfs_base::*;

#[derive(Serialize, Deserialize)]
pub struct RequestResult<T>
{
    pub err: u16,
    pub msg: String,
    pub result: Option<T>
}

impl <T> RequestResult<T>
{
    pub fn from_err(err: BuckyError) -> Self {
        RequestResult {
            err: err.code().into(),
            msg: format!("{}", err),
            result: None
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, RawEncode, RawDecode, Eq, PartialEq)]
pub struct SPVTx {
    pub hash: String,
    pub number: i64,
    pub from: String,
    pub to: String,
    pub coin_id: u8,
    pub value: i64,
    pub desc: String,
    pub create_time: i64,
    pub result: u32,
    pub use_fee: u32,
    pub nonce: i64,
    pub gas_coin_id : u8,//用哪种coin来支付手续费
    pub gas_price : u16, //
    pub max_fee : u32,
}

impl <T> From<T> for RequestResult<T>
{
    fn from(result: T) -> Self {
        RequestResult {
            err: 0,
            msg: "".to_string(),
            result: Some(result)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct GetTxListRequest {
    pub address_list: Vec<String>,
    pub block_section: Option<(i64, i64)>,
    pub offset: i64,
    pub length: i64,
    pub coin_id_list: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlocksRequest {
    pub start_block: i64,
    pub end_block: i64,
}

pub const TX_STATUS_PENDING: u8 = 1;
pub const TX_STATUS_BLOCKED: u8 = 2;

#[derive(Serialize, Deserialize)]
pub struct TxMetaData {
    pub tx_hash: String,
    pub create_time: String,
    pub nonce : String,
    pub caller: String,
    pub gas_coin_id : u8,//用哪种coin来支付手续费
    pub gas_price : u16, //
    pub max_fee : u32,
    pub result: u32,
    pub use_fee: u32,
    pub to: Vec<(String, u8, String)>
}

#[derive(Serialize, Deserialize)]
pub struct TxInfo {
    pub status: u8,
    pub tx: TxMetaData,
    pub block_number: Option<String>,
    pub block_hash: Option<String>,
    pub block_create_time: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct BlockInfo {
    pub height: i64,
    pub block_hash: String,
    pub create_time: u64,
    pub tx_list: Vec<TxInfo>
}

#[derive(Serialize, Deserialize)]
pub struct ERC20ContractTx {
    pub address: String,
    pub tx_hash: String,
    pub start_number: u64,
    pub end_number: u64,
    pub from: String,
    pub to: String,
}

#[derive(Serialize, Deserialize, RawDecode, RawEncode, Clone)]
pub struct ERC20ContractTxResponse {
    pub address: String,
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    pub value: u64,
    pub height: u64,
    pub gas_price : u64,
    pub create_time: u64,
    pub result: i32,
}
