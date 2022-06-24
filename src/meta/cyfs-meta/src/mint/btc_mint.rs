use crate::state_storage::{StateWeakRef, StateRef};
use std::rc::{Rc, Weak};
use crate::executor::context::{ConfigWeakRef, ConfigRef};
use crate::helper::{ArcWeakHelper};
use serde::{Serialize, Deserialize};
use log::*;
use http_types::{Request, Method, Url};
use async_std::net::TcpStream;
use cyfs_base::*;
use crate::*;
use std::time::Duration;
use std::fmt::Display;

pub type BTCMintRef = Rc<BTCMint>;
pub type BTCMintWeakRef = Weak<BTCMint>;

pub struct BTCMint {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    btc_node_url: String,
}

#[derive(RawEncode, RawDecode, Serialize, Deserialize)]
struct GetHeightResp {
    err: i64,
    height: u64
}

#[derive(RawEncode, RawDecode, Serialize, Deserialize)]
struct GetTxResp {
    err: i64,
    height: u64,
    list: Vec<BTCTxRecord>
}

impl BTCMint {
    pub fn new(ref_state: &StateRef, config: &ConfigRef, btc_node_url: &str) -> BTCMintRef {
        BTCMintRef::new(BTCMint {
            ref_state: StateRef::downgrade(ref_state),
            btc_node_url: btc_node_url.to_owned(),
            config: ConfigRef::downgrade(config),
        })
    }

    fn map_err<T, E: Display>(e: E) -> BuckyResult<T> {
        error!("error:{}", e);
        Err(meta_err!(ERROR_EXCEPTION))
    }

    fn map_option<E>(o: Option<E>) -> BuckyResult<E> {
        if o.is_none() {
            Err(meta_err!(ERROR_EXCEPTION))
        } else {
            Ok(o.unwrap())
        }
    }

    fn http_request(&self, url: &str) -> BuckyResult<String> {
        // reqwest::get(url).or_else(BTCMint::map_err)?.text().or_else(BTCMint::map_err)
        async_std::task::block_on(async {
            let url_obj = Url::parse(url).unwrap();
            let host = url_obj.host().unwrap().to_string();
            let mut port = 80;
            if url_obj.port().is_some() {
                port = url_obj.port().unwrap();
            }
            let req = Request::new(Method::Get, url_obj);
            let addr = format!("{}:{}", host, port);
            let stream = TcpStream::connect(addr).await.map_err(|err| {
                error!("connect to failed! miner={}, err={}", host, err);
                meta_err!(ERROR_EXCEPTION)
            })?;

            let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
                error!("http connect error! host={}, err={}", host, err);
                meta_err!(ERROR_EXCEPTION)
            })?;

            resp.body_string().await.map_err(|err| {
                error!("recv body error! err={}", err);
                meta_err!(ERROR_EXCEPTION)
            })
        })
    }

    fn to_obj<'a, T: Deserialize<'a>>(&self, data: &'a str) -> BuckyResult<T> {
        serde_json::from_str(data).or_else(BTCMint::map_err)
    }

    fn get_cur_block_height(&self) -> BuckyResult<u64> {
        if self.btc_node_url.is_empty() {
            return Err(meta_err!(ERROR_EXCEPTION));
        }
        let url = format!("{}/bfc/coinbase/height", self.btc_node_url);
        let resp = self.to_obj::<GetHeightResp>(self.http_request(url.as_str())?.as_str())?;
        if resp.err == 0 {
            Ok(resp.height)
        } else {
            Err(meta_err!(ERROR_EXCEPTION))
        }
    }

    fn query_all_tx(&self, block_begin: u64, block_end: u64) -> BuckyResult<GetTxResp> {
        if self.btc_node_url.is_empty() {
            return Err(meta_err!(ERROR_EXCEPTION));
        }
        let url = format!("{}/bfc/coinbase/blocks?from={}&to={}", self.btc_node_url, block_begin, block_end);
        let resp = self.to_obj::<GetTxResp>(self.http_request(url.as_str())?.as_str())?;
        if resp.err == 0 {
            Ok(resp)
        } else {
            Err(meta_err!(ERROR_EXCEPTION))
        }
    }

    fn query_address_tx(&self, txid: &str, address: &str) -> BuckyResult<GetTxResp> {
        if self.btc_node_url.is_empty() {
            return Err(meta_err!(ERROR_EXCEPTION));
        }
        let url = format!("{}/bfc/coinbase/txs?txid={}&address={}", self.btc_node_url, txid, address);
        let resp = self.to_obj::<GetTxResp>(self.http_request(url.as_str())?.as_str())?;
        if resp.err == 0 {
            Ok(resp)
        } else {
            Err(meta_err!(ERROR_EXCEPTION))
        }
    }

    pub fn create_btc_coinage_record_tx(&self) -> BuckyResult<BTCCoinageRecordTx> {
        let cur_height = self.get_cur_block_height()?;
        let latest_height = self.config.to_rc()?.get_btc_latest_height()?;
        if cur_height == latest_height {
            return Err(meta_err!(ERROR_HEIGHT_NOT_CHANGE));
        }

        let resp = self.query_all_tx(latest_height + 1, cur_height)?;

        Ok(BTCCoinageRecordTx {
            height: cur_height,
            list: resp.list.clone()
        })
    }

    pub fn create_btc_genesis_tx(&self) -> BuckyResult<BTCCoinageRecordTx> {
        let cur_height = self.get_cur_block_height()?;
        Ok(BTCCoinageRecordTx {
            height: cur_height,
            list: Vec::new()
        })
    }

    pub fn check_btc_coinage_record(&self, tx: &BTCCoinageRecordTx) -> BuckyResult<bool> {
        let latest_height = self.config.to_rc()?.get_btc_latest_height()?;
        if 0 == latest_height && tx.list.len() == 0 {
            return Ok(true);
        }
        if latest_height >= tx.height {
            return Ok(false);
        }

        loop {
            let ret = self.get_cur_block_height();
            if ret.is_ok() && ret.unwrap() >= tx.height {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }

        loop {
            let resp = self.query_all_tx(latest_height + 1, tx.height);
            if resp.is_ok() {
                let resp = resp.unwrap();
                if resp.list.len() != tx.list.len() {
                    return Ok(false);
                }

                for i in 0..resp.list.len() {
                    let record = &resp.list[i];
                    if record != tx.list.get(i).unwrap() {
                        return Ok(false);
                    }
                }

                return Ok(true)
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub async fn execute_btc_coinage_record(&self, tx: &BTCCoinageRecordTx) -> BuckyResult<()> {
        self.config.to_rc()?.set_btc_latest_height(tx.height).await?;

        for record in tx.list.as_slice() {
            let address = ObjectId::clone_from_hex(record.address.as_str(), &mut Vec::new())?;
            let btc_value = record.btcValue;

            let price = btc_value * self.config.to_rc()?.btc_exchange_rate();
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(self.config.to_rc()?.default_coin_id()), &address, price as i64).await?;
        }
        Ok(())
    }
}
