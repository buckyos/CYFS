use serde::{Serialize, Deserialize};
//use std::collections::{btree_map};

use cyfs_base::*;

type PreBalance = (ObjectId, i64);
#[derive(Serialize, Deserialize, RawEncode, RawDecode)]
pub struct GenesisCoinConfig {
    pub coin_id: u8,
    pub pre_balance: Vec<PreBalance>
}

#[derive(Serialize, Deserialize, RawEncode, RawDecode)]
pub struct GenesisPriceConfig {
}

#[derive(Serialize, Deserialize, RawEncode, RawDecode)]
#[cyfs(optimize_option)]
pub struct GenesisConfig {
    pub chain_type: Option<String>,
    pub coinbase: ObjectId,
    pub interval: u32,
    pub bfc_spv_node: String,
    pub coins: Vec<GenesisCoinConfig>,
    pub price: GenesisPriceConfig,
    pub miner_key_path: Option<String>,
    pub mg_path: Option<String>,
    pub miner_desc_path: Option<String>,
    pub sub_chain_tx: Option<String>,
}

