use crate::{StateWeakRef, StateRef, ArcWeakHelper};
use std::rc::{Rc, Weak};
use cyfs_base::*;
use http_types::{Method, Request, Url};
use async_std::net::TcpStream;
use serde::Deserialize;
use cyfs_base_meta::*;
use crate::executor::context::{ConfigWeakRef, ConfigRef};
use std::time::Duration;
use std::str::FromStr;
use crate::*;
use log::*;
use std::fmt::Display;

pub struct SubChainMint {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    main_chain_node_url: String,
    miner_group_id: ObjectId,
}
pub type SubChainMintRef = Rc<SubChainMint>;
pub type SubChainMintWeakRef = Weak<SubChainMint>;

impl SubChainMint {
    pub fn new(miner_group_id: ObjectId, ref_state: &StateRef, config: &ConfigRef, main_chain_node_url: String) -> SubChainMintRef {
        SubChainMintRef::new(SubChainMint {
            ref_state: StateRef::downgrade(ref_state),
            config: ConfigRef::downgrade(config),
            main_chain_node_url,
            miner_group_id
        })
    }

    async fn http_get_request(&self, url: &str) -> BuckyResult<String> {
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
    }

    async fn http_post_request(&self, url: &str, param: &str) -> BuckyResult<String> {
        let url_obj = Url::parse(url).unwrap();
        let host = url_obj.host().unwrap().to_string();
        let mut port = 80;
        if url_obj.port().is_some() {
            port = url_obj.port().unwrap();
        }
        let mut req = Request::new(Method::Post, url_obj);
        req.set_body(param);
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
    }

    fn map_err<T, E: Display>(e: E) -> BuckyResult<T> {
        error!("error:{}", e);
        Err(meta_err!(ERROR_EXCEPTION))
    }

    fn to_obj<'a, T: Deserialize<'a>>(&self, data: &'a str) -> BuckyResult<T> {
        serde_json::from_str(data).or_else(SubChainMint::map_err)
    }

    async fn get_cur_block_height(&self) -> BuckyResult<i64> {
        if self.main_chain_node_url.is_empty() {
            return Err(meta_err!(ERROR_EXCEPTION));
        }

        let url = format!("{}/status", self.main_chain_node_url);
        let res = self.http_get_request(url.as_str()).await?;
        let ret = self.to_obj::<RequestResult<ChainStatus>>(res.as_str())?;
        if ret.err == 0 {
            Ok(ret.result.unwrap().height)
        } else {
            Err(meta_err!(ret.err as u32))
        }
    }

    async fn query_all_tx(&self, block_begin: i64, block_end: i64) -> BuckyResult<Vec<SPVTx>> {
        if self.main_chain_node_url.is_empty() {
            return Err(meta_err!(ERROR_EXCEPTION));
        }

        let mut coin_ids = Vec::new();
        for i in u8::MIN..u8::MAX {
            let buf = &[i];
            let str = String::from_utf8_lossy(buf).into_owned();
            coin_ids.push(str);
        }

        let url = format!("{}/collect_tx_list", self.main_chain_node_url);
        let req = GetTxListRequest {
            address_list: vec![self.miner_group_id.to_string()],
            block_section: Some((block_begin, block_end)),
            offset: 0,
            length: i64::max_value(),
            coin_id_list: coin_ids
        };

        let res = self.http_post_request(url.as_str(),
        serde_json::to_string(&req).unwrap().as_str()).await?;
        let ret = self.to_obj::<RequestResult<Vec<SPVTx>>>(res.as_str())?;
        if ret.err == 0 {
            Ok(ret.result.unwrap())
        } else {
            Err(meta_err!(ret.err as u32))
        }
    }

    pub async fn create_genesis_tx(&self) -> BuckyResult<SubChainCoinageRecordTx> {
        let height = self.get_cur_block_height().await?;
        Ok(SubChainCoinageRecordTx {
            height,
            list: vec![]
        })
    }

    pub async fn create_coinage_record_tx(&self) -> BuckyResult<SubChainCoinageRecordTx> {
        let cur_height = self.get_cur_block_height().await?;
        let latest_height = self.config.to_rc()?.get_main_chain_latest_height()?;
        if cur_height == latest_height {
            return Err(meta_err!(ERROR_HEIGHT_NOT_CHANGE));
        }

        let resp = self.query_all_tx(latest_height + 1, cur_height).await?;
        let mut list = Vec::new();
        for tx in &resp {
            if tx.result == 0 {
                list.push(tx.clone());
            }
        }

        Ok(SubChainCoinageRecordTx {
            height: cur_height,
            list
        })
    }

    pub async fn check_coinage_record(&self, tx: &SubChainCoinageRecordTx) -> BuckyResult<bool> {
        let latest_height = self.config.to_rc()?.get_main_chain_latest_height()?;
        if 0 == latest_height && tx.list.len() == 0 {
            return Ok(true);
        }
        if latest_height >= tx.height {
            return Ok(false);
        }

        loop {
            let ret = self.get_cur_block_height().await;
            if ret.is_ok() && ret.unwrap() >= tx.height {
                break;
            }
            async_std::task::sleep(Duration::from_secs(1)).await
        }

        loop {
            let resp = self.query_all_tx(latest_height + 1, tx.height).await;
            if resp.is_ok() {
                let resp = resp.unwrap();
                if resp.len() != tx.list.len() {
                    return Ok(false);
                }

                for i in 0..resp.len() {
                    let record = &resp[i];
                    if record != tx.list.get(i).unwrap() {
                        return Ok(false);
                    }
                }

                return Ok(true)
            }
            async_std::task::sleep(Duration::from_secs(1)).await
        }
    }

    pub async fn execute_coinage_record(&self, tx: &SubChainCoinageRecordTx) -> BuckyResult<()> {
        self.config.to_rc()?.set_main_chain_latest_height(tx.height).await?;

        for record in tx.list.as_slice() {
            let address = ObjectId::from_str(record.from.as_str())?;
            let value = record.value;
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(record.coin_id), &address, value).await?;
        }

        Ok(())
    }
}
