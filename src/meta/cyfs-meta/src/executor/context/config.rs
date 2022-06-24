use crate::state_storage::{StateRef, StateWeakRef};
use log::*;
use cyfs_base::{CoinTokenId, BuckyResult};
use std::str::FromStr;
use std::fmt::Display;
use crate::helper::{ArcWeakHelper};
use std::sync::{Arc, Weak};
use crate::State;
use crate::*;

pub struct Config {
    min_gas_price: u16,
    min_desc_price: u32,
    min_name_rent_price: u32,
    min_name_buy_price: u64,
    buy_name_coin_id: u8,
    name_rent_coin_id: u8,
    get_rent_cycle: i64, //租金周期
    min_auction_stop_interval: u16, //用户出价之后到拍卖完成的最小间隔
    max_auction_stop_interval: u16, //拍卖最大间隔
    min_auction_price: i64, //每次拍卖出价的最小新增价格
    arrears_auction_wait_interval: u16, //欠费拍卖最终拍卖确认间隔
    ref_state: StateWeakRef,
    btc_exchange_rate: u64,
    default_coin_id: u8,
    name_rent_arrears_auctioned_interval: u32, //名字欠费到被拍卖的间隔
    union_withdraw_interval: u32,
    miner_list: Vec<String>,
    nft_apply_buy_time: u64,
}

pub type ConfigRef = Arc<Config>;
pub type ConfigWeakRef = Weak<Config>;

impl Config {
    fn get<T: FromStr>(ref_state: &StateRef, key: &str, default: &str) -> BuckyResult<T>
    where <T as std::str::FromStr>::Err : Display
    {
        let ret_str = async_std::task::block_on(async {ref_state.config_get(key, default).await}).map_err(|e| {
            error!("get config {} err {}", key, e);
            e
        })?;
        ret_str.parse::<T>().map_err(|e| crate::meta_err!({
            error!("parse config {} value {} err {}", key, &ret_str, e);
            ERROR_EXCEPTION
        }))
    }

    pub fn new(ref_state: &StateRef) -> BuckyResult<ConfigRef> {
        let min_gas_price:u16 = Config::get(ref_state, "min_gas_price", "0")?;
        let min_desc_price:u32 = Config::get(ref_state,"min_desc_price", "0")?;
        let min_name_rent_price:u32 = Config::get(ref_state,"min_name_rent_price", "0")?;
        let min_name_buy_price:u64 = Config::get(ref_state,"min_name_buy_price", "0")?;
        let buy_name_coin_id:u8 = Config::get(ref_state,"buy_name_coin_id", "0")?;
        let name_rent_coin_id:u8 = Config::get(ref_state,"name_rent_coin_id", "0")?;
        let get_rent_cycle:i64 = Config::get(ref_state,"rent_cycle", "100")?;
        let min_auction_stop_interval: u16 = Config::get(ref_state, "min_auction_stop_interval", "1")?;
        let max_auction_stop_interval: u16 = Config::get(ref_state, "max_auction_stop_interval", "10")?;
        let min_auction_price: i64 = Config::get(ref_state, "min_auction_price", "0")?;
        let arrears_auction_wait_interval: u16 = Config::get(ref_state, "arrears_auction_wait_interval", "10")?;
        let btc_exchange_rate: u64 = Config::get(ref_state, "btc_exchange_rate", "100000")?;
        let default_coin_id:u8 = Config::get(ref_state,"default_coin_id", "0")?;
        let name_rent_arrears_auctioned_interval: u32 = Config::get(ref_state, "name_rent_arrears_auctioned_interval", "5")?;
        let union_withdraw_interval: u32 = Config::get(ref_state, "union_withdraw_interval", "10")?;
        let miner_list: Vec<String> = serde_json::from_str(Config::get::<String>(ref_state, "miner_list", "[]")?.as_str()).unwrap();
        let nft_apply_buy_time: u64 = Config::get(ref_state, "nft_apply_buy_time", "12960")?;

        Ok(ConfigRef::new(Config {
            min_gas_price,
            min_desc_price,
            min_name_rent_price,
            min_name_buy_price,
            buy_name_coin_id,
            name_rent_coin_id,
            get_rent_cycle,
            min_auction_stop_interval,
            max_auction_stop_interval,
            min_auction_price,
            arrears_auction_wait_interval,
            btc_exchange_rate,
            default_coin_id,
            ref_state: StateRef::downgrade(ref_state),
            name_rent_arrears_auctioned_interval,
            union_withdraw_interval,
            miner_list,
            nft_apply_buy_time
        }))
    }

    pub fn min_gas_price(&self) -> u16 {
        self.min_gas_price
    }

    pub fn min_desc_price(&self, _ctid: &CoinTokenId) -> u32{
        self.min_desc_price
    }

    pub fn min_name_rent_price(&self) -> u32{
        self.min_name_rent_price
    }

    pub fn min_name_buy_price(&self, _name: &str) -> u64{
        self.min_name_buy_price
    }

    pub fn buy_name_coin_id(&self, _name: &str) -> u8{
        self.buy_name_coin_id
    }

    pub fn name_rent_coin_id(&self) -> u8 {
        self.name_rent_coin_id
    }

    pub fn get_rent_cycle(&self) -> i64 {
        self.get_rent_cycle
    }

    pub fn min_auction_stop_interval(&self) -> i64 {
        self.min_auction_stop_interval as i64
    }

    pub fn max_auction_stop_interval(&self) -> i64 {
        self.max_auction_stop_interval as i64
    }

    pub fn min_auction_price(&self) -> i64 {
        self.min_auction_price
    }

    pub fn arrears_auction_wait_interval(&self) -> i64 {
        self.arrears_auction_wait_interval as i64
    }

    pub fn btc_exchange_rate(&self) -> u64 {
        self.btc_exchange_rate
    }

    pub fn name_rent_arrears_auctioned_interval(&self) -> u32 {
        self.name_rent_arrears_auctioned_interval
    }

    pub async fn set_btc_latest_height(&self, height: u64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.config_set("btc_height", format!("{}", height).as_str()).await
    }

    pub fn default_coin_id(&self) -> u8 {
        self.default_coin_id
    }

    pub fn get_btc_latest_height(&self) -> BuckyResult<u64> {
        let height: u64 = Config::get(&self.ref_state.to_rc()?, "btc_height", "0")?;
        Ok(height)
    }

    pub fn union_withdraw_interval(&self) -> u32 {
        self.union_withdraw_interval
    }

    pub fn get_miner_list(&self) -> &Vec<String> {
        &self.miner_list
    }

    pub async fn set_main_chain_latest_height(&self, height: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.config_set("main_chain_height", format!("{}", height).as_str()).await
    }

    pub fn get_main_chain_latest_height(&self) -> BuckyResult<i64> {
        let height: i64 = Config::get(&self.ref_state.to_rc()?, "main_chain_height", "0")?;
        Ok(height)
    }

    pub fn nft_apply_buy_time(&self) -> BuckyResult<u64> {
        Ok(self.nft_apply_buy_time)
    }
}
