use cyfs_base::*;
use cyfs_base_meta::*;

use crate::MetaMinerTarget;

use async_std::net::TcpStream;
use http_types::{Method, Request, Url};
use log::*;
use serde_json::Value;
use std::convert::TryFrom;
use primitive_types::H256;

pub struct MetaClient {
    miner_host: Url,
}

pub const UNION_ACCOUNT_TYPE_CHUNK_PROOF: u8 = 0;
pub const UNION_ACCOUNT_TYPE_SN_PROOF: u8 = 1;
pub const UNION_ACCOUNT_TYPE_DNS_PROOF: u8 = 2;

impl MetaClient {
    pub fn new_target(target: MetaMinerTarget) -> Self {
        let url = target.miner_url();

        info!("will select meta service url: target={}, url={}", target.to_string(), &url);
        Self::new(&url)
    }

    pub fn new(miner_host: &str) -> Self {
        let mut host = miner_host.to_owned();
        if !host.ends_with("/") {
            host = host + "/";
        }

        Self {
            miner_host: Url::parse(&host).unwrap(),
        }
    }

    fn gen_url(&self, path: &str) -> Url {
        self.miner_host.join(path).unwrap()
    }

    pub async fn get_balance(&self, account: &ObjectId, coin_id: u8) -> BuckyResult<ViewBalanceResult> {
        let req = self.get_balance_request(account, coin_id);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewBalance(br) = resp {
            Ok(br)
        } else {
            Err(BuckyError::new(BuckyErrorCode::Failed, "view failed"))
        }
    }

    pub async fn trans(
        &self,
        from: &StandardObject,
        to: &ObjectId,
        v: i64,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self.trans_request(from, to, v, coin_id, secret).await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn trans_ex(
        &self,
        from: TxCaller,
        to: &ObjectId,
        v: i64,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self.trans_request_ex(from, to, v, coin_id, secret).await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn trans_request(
        &self,
        from: &StandardObject,
        to: &ObjectId,
        v: i64,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request(
            from,
            secret,
            MetaTxBody::TransBalance(TransBalanceTx {
                ctid: CoinTokenId::Coin(coin_id),
                to: vec![(to.clone(), v)],
            }),
            10,10,
            Vec::new(),
        )
        .await
    }

    pub async fn trans_request_ex(
        &self,
        from: TxCaller,
        to: &ObjectId,
        v: i64,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request_ex(
            from,
            secret,
            MetaTxBody::TransBalance(TransBalanceTx {
                ctid: CoinTokenId::Coin(coin_id),
                to: vec![(to.clone(), v)],
            }),
            10,10,
            Vec::new(),
        )
        .await
    }

    async fn create_tx_and_sign(
        &self,
        caller: &StandardObject,
        secret: &PrivateKey,
        body: MetaTxBody,
        gas_price: u16,
        max_fee: u32,
        tx_data: Vec<u8>,
    ) -> BuckyResult<MetaTx> {
        let caller = TxCaller::try_from(caller)?;
        self.create_tx_and_sign_ex(caller, secret, body, gas_price, max_fee, tx_data).await
    }

    async fn create_tx_and_sign_ex(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        body: MetaTxBody,
        gas_price: u16,
        max_fee: u32,
        tx_data: Vec<u8>,
    ) -> BuckyResult<MetaTx> {
        let mut nonce = self.get_nonce(&caller.id()?).await?;
        nonce += 1;

        let mut tx = MetaTx::new(nonce, caller, 0, gas_price, max_fee, None, body, tx_data).build();
        let signer = RsaCPUObjectSigner::new(secret.public(), secret.clone());
        sign_and_set_named_object(&signer, &mut tx, &SignatureSource::RefIndex(0)).await?;
        Ok(tx)
    }

    pub async fn create_tx_not_sign(
        &self,
        caller: TxCaller,
        body: MetaTxBody,
        gas_price: u16,
        max_fee: u32,
        tx_data: Vec<u8>,
    ) -> BuckyResult<Tx> {
        let mut nonce = self.get_nonce(&caller.id()?).await?;
        nonce += 1;
        let tx = Tx::new(nonce, caller, 0, gas_price, max_fee, None, vec![body].to_vec()?, tx_data).build();
        Ok(tx)
    }

    pub async fn create_tx_not_sign2(
        &self,
        caller: TxCaller,
        bodys: Vec<MetaTxBody>,
        gas_price: u16,
        max_fee: u32,
        tx_data: Vec<u8>,
    ) -> BuckyResult<Tx> {
        let mut nonce = self.get_nonce(&caller.id()?).await?;
        nonce += 1;
        let tx = Tx::new(nonce, caller, 0, gas_price, max_fee, None, bodys.to_vec()?, tx_data).build();
        Ok(tx)
    }

    fn commit_signed_tx(&self, tx: MetaTx) -> BuckyResult<Request> {
        let body = tx.encode_to_vec(true)?;
        let url = self.gen_url("commit");
        let mut request = Request::new(Method::Post, url);
        request.set_body(body);
        Ok(request)
    }

    async fn commit_request(
        &self,
        caller: &StandardObject,
        secret: &PrivateKey,
        body: MetaTxBody,
        gas_price: u16,
        max_fee: u32,
        data: Vec<u8>,
    ) -> BuckyResult<Request> {
        let signed_tx = self.create_tx_and_sign(caller, secret, body, gas_price, max_fee, data).await?;
        self.commit_signed_tx(signed_tx)
    }

    async fn commit_request_ex(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        body: MetaTxBody,
        gas_price: u16,
        max_fee: u32,
        tx_data: Vec<u8>,
    ) -> BuckyResult<Request> {
        let signed_tx = self
            .create_tx_and_sign_ex(caller, secret, body, gas_price, max_fee, tx_data)
            .await?;
        self.commit_signed_tx(signed_tx)
    }

    pub fn get_balance_request(&self, account: &ObjectId, coin_id: u8) -> Request {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewBalance(ViewBalanceMethod {
                account: account.clone(),
                ctid: vec![CoinTokenId::Coin(coin_id)],
            }),
        };
        self.view_request(view)
    }

    pub fn view_request(&self, view: ViewRequest) -> Request {
        let mut req = Request::new(Method::Post, self.gen_url("view"));
        req.set_body(view.to_hex().unwrap());
        req
    }

    pub async fn get_desc(&self, id: &ObjectId) -> BuckyResult<SavedMetaObject> {
        let req = self.get_desc_request(id);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;

        if let ViewResponse::ViewDesc(desc) = resp {
            Ok(desc)
        } else {
            Err(BuckyError::from("get desc failed"))
        }
    }

    pub fn get_raw_request(&self, id: &ObjectId) -> Request {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewRaw(ViewRawMethod { id: id.clone() }),
        };
        self.view_request(view)
    }

    pub async fn get_raw_data(&self, id: &ObjectId) -> BuckyResult<Vec<u8>> {
        let req = self.get_raw_request(id);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;

        if let ViewResponse::ViewRaw(desc) = resp {
            Ok(desc.into())
        } else {
            Err(BuckyError::from("get desc failed"))
        }
    }

    pub async fn get_chain_status(&self) -> BuckyResult<ChainStatus> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewStatus,
        };
        let request = self.view_request(view);
        let resp: ViewResponse = self.request_miner(request, &mut Vec::new()).await?;
        if let ViewResponse::ViewStatus(chain_status) = resp {
            Ok(chain_status)
        } else {
            Err(BuckyError::from("get chain status failed"))
        }
    }

    pub async fn get_block(&self, height: i64) -> BuckyResult<Block> {
        let view = ViewRequest {
            block: ViewBlockEnum::Number(height),
            method: ViewMethodEnum::ViewBlock,
        };
        let request = self.view_request(view);
        let resp: ViewResponse = self.request_miner(request, &mut Vec::new()).await?;
        if let ViewResponse::ViewBlock(block) = resp {
            Ok(block)
        } else {
            Err(BuckyError::from("get block failed"))
        }
    }

    pub fn get_desc_request(&self, id: &ObjectId) -> Request {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewDesc(ViewDescMethod { id: id.clone() }),
        };
        self.view_request(view)
    }

    pub async fn create_desc(
        &self,
        owner: &StandardObject,
        desc: &SavedMetaObject,
        v: i64,
        price: u32,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .create_desc_request(owner, desc, v, coin_id, price, secret)
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn create_desc_ex(
        &self,
        caller: TxCaller,
        desc: &SavedMetaObject,
        v: i64,
        price: u32,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .create_desc_request_ex(caller, desc, v, coin_id, price, secret)
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn create_desc_request(
        &self,
        owner: &StandardObject,
        desc: &SavedMetaObject,
        v: i64,
        coin_id: u8,
        price: u32,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request(
            owner,
            secret,
            MetaTxBody::CreateDesc(CreateDescTx {
                coin_id,
                from: None,
                value: v,
                desc_hash: desc.hash()?,
                price,
            }),
            10,10,
            desc.to_vec()?,
        )
        .await
    }

    pub async fn create_desc_request_ex(
        &self,
        owner: TxCaller,
        desc: &SavedMetaObject,
        v: i64,
        coin_id: u8,
        price: u32,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request_ex(
            owner,
            secret,
            MetaTxBody::CreateDesc(CreateDescTx {
                coin_id,
                from: None,
                value: v,
                desc_hash: desc.hash()?,
                price,
            }),
            10,10,
            desc.to_vec()?,
        )
        .await
    }

    pub async fn update_desc(
        &self,
        owner: &StandardObject,
        desc: &SavedMetaObject,
        price: Option<u32>,
        coin_id: Option<u8>,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self.update_desc_request(owner, desc, price, coin_id, secret).await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn update_desc_ex(
        &self,
        owner: TxCaller,
        desc: &SavedMetaObject,
        price: Option<u32>,
        coin_id: Option<u8>,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .update_desc_request_ex(owner, desc, price, coin_id, secret)
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn update_desc_request(
        &self,
        owner: &StandardObject,
        desc: &SavedMetaObject,
        price: Option<u32>,
        coin_id: Option<u8>,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request(
            owner,
            secret,
            MetaTxBody::UpdateDesc(UpdateDescTx {
                write_flag: 0,
                price: price.map(|p| MetaPrice {
                    coin_id: coin_id.unwrap(),
                    price: p,
                }),
                desc_hash: desc.hash()?,
            }),
            10,10,
            desc.to_vec()?,
        )
        .await
    }

    pub async fn update_desc_request_ex(
        &self,
        owner: TxCaller,
        desc: &SavedMetaObject,
        price: Option<u32>,
        coin_id: Option<u8>,
        secret: &PrivateKey,
    ) -> BuckyResult<Request> {
        self.commit_request_ex(
            owner,
            secret,
            MetaTxBody::UpdateDesc(UpdateDescTx {
                write_flag: 0,
                price: price.map(|p| MetaPrice {
                    coin_id: coin_id.unwrap(),
                    price: p,
                }),
                desc_hash: desc.hash()?,
            }),
            10,10,
            desc.to_vec()?,
        )
        .await
    }

    pub async fn bid_name(
        &self,
        caller: &StandardObject,
        owner: Option<ObjectId>,
        name: &str,
        price: u64,
        rent: u32,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::BidName(BidNameTx {
                    name: name.to_owned(),
                    owner: owner.map(|id| id),
                    name_price: price,
                    price: rent,
                }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn get_name(&self, name: &str) -> BuckyResult<Option<(NameInfo, NameState)>> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewName(ViewNameMethod {
                name: name.to_owned(),
            }),
        };
        let _body = view.to_hex()?;
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;

        if let ViewResponse::ViewName(desc) = resp {
            Ok(desc)
        } else {
            Err(BuckyError::from("get name info failed"))
        }
    }

    pub async fn update_name(
        &self,
        caller: &StandardObject,
        name: &str,
        info: NameInfo,
        write_flag: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::UpdateName(UpdateNameTx {
                    name: name.to_owned(),
                    info,
                    write_flag,
                }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn trans_name(
        &self,
        caller: &StandardObject,
        new_owner: &ObjectId,
        sub_name: Option<&str>,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::TransName(TransNameTx {
                    sub_name: sub_name.map(|str| str.to_owned()),
                    new_owner: new_owner.clone(),
                }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }
    //
    // pub async fn change_name_state(&self, caller: &Device, name: &str, new_state: NameState, secret: &PrivateKey) -> BuckyResult<TxId> {
    //     let req = self.commit_request(caller, secret, MetaTxBody::ChangeNameState(ChangeNameStateTx{ name: name.to_owned(), new_state })).await?;
    //     self.request_miner(req).await
    // }

    pub async fn get_tx_receipt(&self, tx_hash: &TxId) -> BuckyResult<Option<(Receipt, i64)>> {
        let req = self.get_receipt_request(tx_hash);
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn get_tx(&self, tx_hash: &TxId) -> BuckyResult<TxInfo> {
        let url = self
            .miner_host
            .join("tx/")
            .unwrap()
            .join(&tx_hash.to_string())
            .unwrap();

        let req = Request::new(Method::Get, url);

        debug!("miner request url={}", req.url());

        let host = self.miner_host.host_str().unwrap();
        let port = self.miner_host.port().unwrap_or(80);
        let addr = format!("{}:{}", host, port);

        let stream = TcpStream::connect(addr).await.map_err(|err| {
            error!(
                "connect to miner failed! miner={}, err={}",
                self.miner_host, err
            );
            err
        })?;

        let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
            error!("http connect error! host={}, err={}", self.miner_host, err);
            err
        })?;
        let ret: Value = resp.body_json().await?;
        if 0 == ret.get("err").unwrap().as_u64().unwrap() {
            Ok(serde_json::from_value(ret.get("result").unwrap().clone())?)
        } else {
            Err(BuckyError::new(
                ret.get("err").unwrap().as_u64().unwrap() as u16,
                ret.get("msg").unwrap().as_str().unwrap(),
            ))
        }
    }

    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        let req = self.get_nonce_request(account);
        self.request_miner(req, &mut Vec::new()).await
    }

    pub fn get_nonce_request(&self, account: &ObjectId) -> Request {
        let mut url = self.gen_url("nonce");

        let query = format!("id={}", &account);
        url.set_query(Some(&query));

        Request::new(Method::Get, url)
    }

    pub fn get_receipt_request(&self, tx: &TxId) -> Request {
        let mut url = self.gen_url("receipt");

        let query = format!("tx={}", tx.as_ref());
        url.set_query(Some(&query));

        Request::new(Method::Get, url)
    }

    pub async fn send_to_miner(&self, tx: MetaTx) -> BuckyResult<TxId> {
        let req = self.commit_signed_tx(tx)?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn request_miner<'de, T: RawDecode<'de>>(
        &self,
        req: Request,
        buf: &'de mut Vec<u8>,
    ) -> BuckyResult<T> {
        debug!("miner request url={}", req.url());

        let host = self.miner_host.host_str().unwrap();
        let port = self.miner_host.port().unwrap_or(80);
        let addr = format!("{}:{}", host, port);

        let stream = TcpStream::connect(addr).await.map_err(|err| {
            error!(
                "connect to miner failed! miner={}, err={}",
                self.miner_host, err
            );
            err
        })?;

        let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
            error!("http connect error! host={}, err={}", self.miner_host, err);
            err
        })?;
        let ret_hex = resp.body_string()
            .await
            .map_err(|err| {
                error!("recv body error! err={}", err);
                err
            })?;
        let ret = Result::<T, u16>::clone_from_hex(
            ret_hex.as_str(),
            buf,
        )?;

        match ret {
            Ok(t) => Ok(t),
            Err(e) => {
                if e == ERROR_NOT_FOUND {
                    Err(BuckyError::new(BuckyErrorCode::NotFound, "NotFound"))
                } else {
                    Err(BuckyError::new(BuckyErrorCodeEx::MetaError(e), "meta err"))
                }
            }
        }
    }

    pub async fn create_union_account(
        &self,
        caller: &StandardObject,
        create_union_tx: CreateUnionTx,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::CreateUnion(create_union_tx),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn create_deviate_tx(
        &self,
        caller: &StandardObject,
        deviate_tx: DeviateUnionTx,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(caller, secret, MetaTxBody::DeviateUnion(deviate_tx), 10,10, vec![])
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn withdraw_union(
        &self,
        caller: &StandardObject,
        ctid: CoinTokenId,
        union_id: &ObjectId,
        value: i64,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::WithdrawFromUnion(WithdrawFromUnionTx {
                    ctid,
                    union: union_id.clone(),
                    value,
                }),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn set_config(
        &self,
        caller: &StandardObject,
        key: &str,
        value: &str,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::SetConfig(SetConfigTx {
                    key: key.to_owned(),
                    value: value.to_owned(),
                }),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn auction_name(
        &self,
        caller: &StandardObject,
        name: &str,
        startting_price: u64,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::AuctionName(AuctionNameTx {
                    name: name.to_owned(),
                    price: startting_price,
                }),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn cancel_auction_name(
        &self,
        caller: &StandardObject,
        name: &str,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::CancelAuctionName(CancelAuctionNameTx {
                    name: name.to_owned(),
                }),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn buy_back_name(
        &self,
        caller: &StandardObject,
        name: &str,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::BuyBackName(BuyBackNameTx {
                    name: name.to_owned(),
                }),
                10,10,
                vec![],
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn remove_desc(
        &self,
        caller: &StandardObject,
        desc_id: &ObjectId,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request(
                caller,
                secret,
                MetaTxBody::RemoveDesc(RemoveDescTx {
                    id: desc_id.clone(),
                }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn withdraw_from_file(
        &self,
        caller: TxCaller,
        file_id: &ObjectId,
        v: i64,
        coin_id: u8,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::WithdrawToOwner(WithdrawToOwner {
                    ctid: CoinTokenId::Coin(coin_id),
                    id: file_id.clone(),
                    value: v,
                }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn create_sub_chain_account(
        &self,
        caller: TxCaller,
        miner_group: MinerGroup,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::CreateSubChainAccount(miner_group),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn withdraw_from_sub_chain(
        &self,
        caller: TxCaller,
        coin_id: CoinTokenId,
        value: i64,
        secret: &PrivateKey,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::WithdrawFromSubChain(WithdrawFromSubChainTx { coin_id, value }),
                10,10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn commit_extension_tx(
        &self,
        caller: TxCaller,
        extension_tx: MetaExtensionTx,
        secret: &PrivateKey,
        tx_data: Vec<u8>,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(caller, secret, MetaTxBody::Extension(extension_tx), 10,10, tx_data)
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn commit_tx(&self, tx: MetaTx) -> BuckyResult<TxId> {
        let req = self.commit_signed_tx(tx)?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn create_contract(&self, caller: &StandardObject, secret: &PrivateKey, value: u64, init_data: Vec<u8>, gas_price: u16, max_fee: u32) -> BuckyResult<TxId> {
        let req = self.commit_request(
            caller,
            secret,
            MetaTxBody::CreateContract(CreateContractTx {
                value,
                init_data
            }),
            gas_price, max_fee,
            Vec::new(),
        ).await?;
        info!("create request");
        self.request_miner(req, &mut vec![]).await
    }

    pub async fn create_contract2(&self, caller: &StandardObject, secret: &PrivateKey, value: u64, init_data: Vec<u8>, salt: [u8;32], gas_price: u16, max_fee: u32) -> BuckyResult<TxId> {
        let req = self.commit_request(
            caller,
            secret,
            MetaTxBody::CreateContract2(CreateContract2Tx::new(value, init_data, salt)),
            gas_price, max_fee,
            Vec::new(),
        ).await?;
        self.request_miner(req, &mut vec![]).await
    }

    pub async fn call_contract(&self, caller: &StandardObject, secret: &PrivateKey, address: ObjectId, value: u64, data: Vec<u8>, gas_price: u16, max_fee: u32) -> BuckyResult<TxId> {
        let req = self.commit_request(
            caller,
            secret,
            MetaTxBody::CallContract(CallContractTx {
                address,
                value,
                data
            }),
            gas_price, max_fee,
            Vec::new(),
        ).await?;
        self.request_miner(req, &mut vec![]).await
    }

    pub async fn view_contract(&self, address: ObjectId, data: Vec<u8>) -> BuckyResult<ViewContractResult> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewContract(ViewContract {
                address,
                data
            }),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewContract(br) = resp {
            Ok(br)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn set_benefi(&self, address: &ObjectId, benefi: &ObjectId, caller: &StandardObject, secret: &PrivateKey) -> BuckyResult<TxId> {
        let req = self.commit_request(
            caller,
            secret,
            MetaTxBody::SetBenefi(SetBenefiTx {
                address: address.clone(),
                to: benefi.clone()
            }),
            10, 10,
            Vec::new(),
        ).await?;
        self.request_miner(req, &mut vec![]).await
    }

    pub async fn get_benefi(&self, address: &ObjectId) -> BuckyResult<ObjectId> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewBenifi(ViewBenefi {
                address: address.clone(),
            }),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewBenefi(br) = resp {
            Ok(br.address)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn get_logs(&self, address: ObjectId, topics: Vec<Option<H256>>, from: i64, to: i64) -> BuckyResult<Vec<(Vec<H256>, Vec<u8>)>> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewLog(ViewLog {
                address,
                topics,
                from,
                to
            }),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewLog(br) = resp {
            Ok(br.logs)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn nft_create(&self,
                            caller: TxCaller,
                            secret: &PrivateKey,
                            desc: NFTDesc,
                            name: String,
                            state: NFTState) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTCreate(NFTCreateTx {
                    desc,
                    name,
                    state
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_get(
        &self,
        nft_id: ObjectId,
    ) -> BuckyResult<(NFTDesc, String, ObjectId, NFTState)> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewNFT(nft_id),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewNFT(ret) = resp {
            Ok(ret)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn nft_get_apply_buy_list(
        &self,
        nft_id: ObjectId,
        offset: u32,
        length: u8,
    ) -> BuckyResult<ViewNFTBuyListResult> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewNFTApplyBuyList((nft_id, offset, length)),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewNFTApplyBuyList(ret) = resp {
            Ok(ret)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn nft_get_bid_list(
        &self,
        nft_id: ObjectId,
        offset: u32,
        length: u8,
    ) -> BuckyResult<ViewNFTBuyListResult> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewNFTBidList((nft_id, offset, length)),
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewNFTBidList(ret) = resp {
            Ok(ret)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn nft_get_largest_buy_price(
        &self,
        nft_id: ObjectId,
    ) -> BuckyResult<Option<(ObjectId, CoinTokenId, u64)>> {
        let view = ViewRequest {
            block: ViewBlockEnum::Tip,
            method: ViewMethodEnum::ViewNFTLargestBuyValue(nft_id)
        };
        let req = self.view_request(view);
        let resp: ViewResponse = self.request_miner(req, &mut Vec::new()).await?;
        if let ViewResponse::ViewNFTLargestBuyValue(ret) = resp {
            Ok(ret)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotMatch, "view result type not match"))
        }
    }

    pub async fn nft_auction(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        price: u64,
        coin_id: CoinTokenId,
        duration_block_num: u64) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTAuction(NFTAuctionTx {
                    nft_id,
                    price,
                    coin_id,
                    duration_block_num
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_bid(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        price: u64,
        coin_id: CoinTokenId
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTBid(NFTBidTx {
                    nft_id,
                    price,
                    coin_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_buy(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        price: u64,
        coin_id: CoinTokenId
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTBuy(NFTBuyTx {
                    nft_id,
                    price,
                    coin_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_sell(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        price: u64,
        coin_id: CoinTokenId
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTSell(NFTSellTx {
                    nft_id,
                    price,
                    coin_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_apply_buy(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        price: u64,
        coin_id: CoinTokenId
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTApplyBuy(NFTApplyBuyTx {
                    nft_id,
                    price,
                    coin_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_cancel_apply_buy(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTCancelApplyBuyTx(NFTCancelApplyBuyTx {
                    nft_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_agree_apply(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
        user_id: ObjectId,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTAgreeApply(NFTAgreeApplyTx {
                    nft_id,
                    user_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }

    pub async fn nft_like(
        &self,
        caller: TxCaller,
        secret: &PrivateKey,
        nft_id: ObjectId,
    ) -> BuckyResult<TxId> {
        let req = self
            .commit_request_ex(
                caller,
                secret,
                MetaTxBody::NFTLike(NFTLikeTx {
                    nft_id,
                }),
                10, 10,
                Vec::new(),
            )
            .await?;
        self.request_miner(req, &mut Vec::new()).await
    }


    // pub async fn public_sn_service(&self, caller: TxCaller, service: SNService, secret: &PrivateKey) -> BuckyResult<TxId> {
    //     let req = self.commit_request_ex(caller, secret, MetaTxBody::SNService(SNServiceTx::Publish(service)), Vec::new()).await?;
    //     self.request_miner(req, &mut Vec::new()).await
    // }
    //
    // pub async fn purchase_sn_service(&self, caller: TxCaller, contract: Contract, secret: &PrivateKey) -> BuckyResult<TxId> {
    //     let req = self.commit_request_ex(caller, secret, MetaTxBody::SNService(SNServiceTx::Purchase(contract)), Vec::new()).await?;
    //     self.request_miner(req, &mut Vec::new()).await
    // }
    //
    // pub async fn settle_sn_service(&self, caller: TxCaller, proof: ProofOfService, secret: &PrivateKey) -> BuckyResult<TxId> {
    //     let req = self.commit_request_ex(caller, secret, MetaTxBody::SNService(SNServiceTx::Settle(proof)), Vec::new()).await?;
    //     self.request_miner(req, &mut Vec::new()).await
    // }
    //
    // pub async fn get_auth_contract(&self, service_id: &ObjectId, user_id: &ObjectId) -> BuckyResult<Contract> {
    //     let url = self.gen_url(format!("query_auth_contract/{}/{}", service_id.to_string(), user_id.to_string()).as_str());
    //     let req = Request::new(Method::Get, url);
    //     self.request_miner(req, &mut Vec::new()).await
    // }
}


pub struct MetaClientHelper;

impl MetaClientHelper {
    pub async fn get_object(
        meta_client: &MetaClient,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<(AnyNamedObject, Vec<u8>)>> {
        let object_raw = match meta_client.get_raw_data(object_id).await {
            Ok(v) => v,
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    warn!(
                        "get object from meta chain but not found! obj={} err={}",
                        object_id, e
                    );

                    return Ok(None);
                } else {
                    let msg = format!(
                        "load object from meta chain failed! obj={} err={}",
                        object_id, e
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(e.code(), msg));
                }
            }
        };

        info!("get object from meta success: {}", object_id);
        let (object, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
            let msg = format!("invalid object format! obj={} err={}", object_id, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // 校验一下对象id，看是否匹配
        let id = object.calculate_id();
        if id != *object_id {
            let msg = format!(
                "get object from meta but got unmatch object id! expected={}, got={}",
                object_id, id
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        let resp =  (object, object_raw);

        Ok(Some(resp))
    }
}
