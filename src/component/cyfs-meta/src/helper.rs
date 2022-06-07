use std::rc::{Rc};
use cyfs_base::{BuckyError, BuckyResult, BuckyErrorCode, HashValue};
use std::sync::Arc;
use http_types::{Url, Method, Request};
use async_std::net::TcpStream;
use log::*;
use crate::*;
use base58::{FromBase58, ToBase58};

#[macro_export]
macro_rules! meta_err {
    ( $err: expr) => {
    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($err as u16), format!("{} {} dsg_code_err:{}", file!(), line!(), stringify!($err)))
    };
}

#[macro_export]
macro_rules! meta_err2 {
    ( $err: expr, $msg: expr) => {
    cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($err as u16), format!("{} {} msg:{}", file!(), line!(), $msg))
    };
}

#[macro_export]
macro_rules! meta_map_err {
    ( $err: expr, $old_err_code: expr, $new_err_code: expr) => {
        {
            if get_meta_err_code($err)? == $old_err_code {
                cyfs_base::BuckyError::new(cyfs_base::BuckyErrorCodeEx::MetaError($new_err_code as u16), format!("{} {} dsg_code_err:{}", file!(), line!(), $new_err_code))
            } else {
                cyfs_base::BuckyError::new($err.code(), format!("{} {} base_code_err:{}", file!(), line!(), $err))
            }
        }
    }
}

pub trait RcWeakHelper<T: ?Sized> {
    fn to_rc(&self) -> BuckyResult<Rc<T>>;
}

impl <T: ?Sized> RcWeakHelper<T> for std::rc::Weak<T> {
    fn to_rc(&self) -> BuckyResult<Rc<T>> {
        match self.upgrade() {
            Some(v) => {
                Ok(v)
            },
            None => {
                Err(meta_err!(ERROR_EXCEPTION))
            }
        }
    }
}

pub trait ArcWeakHelper<T: ?Sized> {
    fn to_rc(&self) -> BuckyResult<Arc<T>>;
}

impl <T: ?Sized> ArcWeakHelper<T> for std::sync::Weak<T> {
    fn to_rc(&self) -> BuckyResult<Arc<T>> {
        match self.upgrade() {
            Some(v) => {
                Ok(v)
            },
            None => {
                Err(meta_err!(ERROR_EXCEPTION))
            }
        }
    }
}

pub fn get_meta_err_code(ret: &BuckyError) -> BuckyResult<u16> {
    if let BuckyErrorCode::MetaError(code) = ret.code() {
        Ok(code)
    } else {
        Err(meta_err!(ERROR_EXCEPTION))
    }
}

pub async fn http_get_request(url: &str) -> BuckyResult<Vec<u8>> {
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

    resp.body_bytes().await.map_err(|err| {
        error!("recv body error! err={}", err);
        meta_err!(ERROR_EXCEPTION)
    })
}

pub async fn http_post_request(url: &str, param: Vec<u8>) -> BuckyResult<Vec<u8>> {
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

    resp.body_bytes().await.map_err(|err| {
        error!("recv body error! err={}", err);
        meta_err!(ERROR_EXCEPTION)
    })
}

pub(crate) mod sub_chain_helper {
    use http_types::{Request, Url, Method};
    use cyfs_base::*;
    use cyfs_base_meta::*;
    use std::convert::TryFrom;
    use async_std::net::TcpStream;
    use serde_json::Value;
    use log::*;
    use cyfs_base_meta::ERROR_NOT_FOUND;

    pub struct MetaClient {
        miner_host: Url
    }

    pub const UNION_ACCOUNT_TYPE_CHUNK_PROOF: u8  = 0;
    pub const UNION_ACCOUNT_TYPE_SN_PROOF: u8  = 1;
    pub const UNION_ACCOUNT_TYPE_DNS_PROOF: u8  = 2;


    impl MetaClient {
        pub fn new(
            miner_host: &str
        ) -> Self {

            let mut host = miner_host.to_owned();
            if !miner_host.ends_with("/") {
                host = miner_host.to_owned() + "/";
            }

            MetaClient {
                miner_host: Url::parse(&host).unwrap(),
            }
        }

        fn gen_url(&self, path: &str) -> Url {
            self.miner_host.join(path).unwrap()
        }

        async fn create_tx_and_sign(&self, caller: &StandardObject, secret: &PrivateKey, body: MetaTxBody, tx_data: Vec<u8>) -> BuckyResult<Tx> {
            let mut nonce = self.get_nonce(&caller.calculate_id()).await?;
            nonce += 1;

            let mut tx = Tx::new(nonce,
                                 TxCaller::try_from(caller)?,
                                 0,
                                 0,
                                 0,
                                 None,
                                 vec![body].to_vec()?,
                                 tx_data).build();
            let signer = RsaCPUObjectSigner::new(secret.public(), secret.clone());
            sign_and_set_named_object(&signer, &mut tx, &SignatureSource::RefIndex(0)).await?;
            Ok(tx)
        }

        async fn create_tx_and_sign_ex(&self, caller: TxCaller, secret: &PrivateKey, body: MetaTxBody, tx_data: Vec<u8>) -> BuckyResult<Tx> {
            let mut nonce = self.get_nonce(&caller.id()?).await?;
            nonce += 1;

            let mut tx = Tx::new(nonce,
                                 caller,
                                 0,
                                 0,
                                 0,
                                 None,
                                 vec![body].to_vec()?,
                                 tx_data).build();
            let signer = RsaCPUObjectSigner::new(secret.public(), secret.clone());
            sign_and_set_named_object(&signer, &mut tx, &SignatureSource::RefIndex(0)).await?;
            Ok(tx)
        }

        fn commit_signed_tx(&self, tx: Tx) -> BuckyResult<Request> {
            let body = tx.encode_to_vec(true)?;
            let url = self.gen_url("commit");
            let mut request = Request::new(Method::Post, url);
            request.set_body(body);
            Ok(request)
        }

        async fn commit_request(&self, caller: &StandardObject, secret: &PrivateKey, body: MetaTxBody, data: Vec<u8>) -> BuckyResult<Request> {
            let signed_tx = self.create_tx_and_sign(caller, secret, body, data).await?;
            self.commit_signed_tx(signed_tx)
        }

        async fn commit_request_ex(&self, caller: TxCaller, secret: &PrivateKey, body: MetaTxBody, tx_data: Vec<u8>) -> BuckyResult<Request> {
            let signed_tx = self.create_tx_and_sign_ex(caller, secret, body, tx_data).await?;
            self.commit_signed_tx(signed_tx)
        }

        pub fn get_balance_request(&self, account: &ObjectId, coin_id: u8) -> Request {
            let view = ViewRequest {
                block: ViewBlockEnum::Tip,
                method: ViewMethodEnum::ViewBalance(ViewBalanceMethod {
                    account: account.clone(),
                    ctid: vec![CoinTokenId::Coin(coin_id)]
                })
            };
            self.view_request(view)
        }

        fn view_request(&self, view: ViewRequest) -> Request {
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
                method: ViewMethodEnum::ViewRaw(ViewRawMethod {
                    id: id.clone()
                })
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
                method: ViewMethodEnum::ViewDesc(ViewDescMethod {
                    id: id.clone()
                })
            };
            self.view_request(view)
        }

        pub async fn get_tx(&self, tx_hash: &TxId) -> BuckyResult<TxInfo> {
            let url = self.miner_host.join("tx/").unwrap().join(&tx_hash.to_string()).unwrap();

            let req = Request::new(Method::Get, url);

            debug!("miner request url={}", req.url());

            let host = self.miner_host.host_str().unwrap();
            let port = self.miner_host.port().unwrap_or(80);
            let addr = format!("{}:{}", host, port);

            let stream = TcpStream::connect(addr).await.map_err(|err| {
                error!("connect to miner failed! miner={}, err={}", self.miner_host, err);
                err
            })?;

            let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
                error!("http connect error! host={}, err={}", self.miner_host, err);
                err
            })?;
            let ret: Value = resp.body_json().await?;
            if let Some(value) = ret.get("result") {
                Ok(serde_json::from_value(value.clone())?)
            } else {
                Err(BuckyError::new(ret.get("err").unwrap().as_u64().unwrap() as u16, ret.get("msg").unwrap().as_str().unwrap()))
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

        pub async fn send_to_miner(&self, tx: Tx) -> BuckyResult<TxId> {
            let req = self.commit_signed_tx(tx)?;
            self.request_miner(req, &mut Vec::new()).await
        }

        async fn request_miner<'de, T: RawDecode<'de>>(&self, req: Request, buf: &'de mut Vec<u8>) -> BuckyResult<T> {
            debug!("miner request url={}", req.url());

            let host = self.miner_host.host_str().unwrap();
            let port = self.miner_host.port().unwrap_or(80);
            let addr = format!("{}:{}", host, port);

            let stream = TcpStream::connect(addr).await.map_err(|err| {
                error!("connect to miner failed! miner={}, err={}", self.miner_host, err);
                err
            })?;

            let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
                error!("http connect error! host={}, err={}", self.miner_host, err);
                err
            })?;
            let ret = Result::<T, u32>::clone_from_hex(resp.body_string().await.map_err(|err| {
                error!("recv body error! err={}", err);
                err
            })?.as_str(), buf)?;

            match ret {
                Ok(t) => {
                    Ok(t)
                }
                Err(e) => {
                    if e == ERROR_NOT_FOUND as u32 {
                        Err(BuckyError::new(BuckyErrorCode::NotFound, "NotFound"))
                    } else {
                        Err(BuckyError::from(e))
                    }
                }
            }
        }

        pub async fn sub_chain_withdraw(&self, caller: TxCaller, sub_chain_id: ObjectId, withdraw_tx: &MetaTx, secret: &PrivateKey) -> BuckyResult<TxId> {
            let req = self.commit_request_ex(caller, secret, MetaTxBody::SubChainWithdraw(SubChainWithdrawTx{
                subchain_id: sub_chain_id,
                withdraw_tx: withdraw_tx.to_vec()?
            }), Vec::new()).await?;
            self.request_miner(req, &mut Vec::new()).await
        }

    }

}

pub trait HashValueEx {
    fn to_base58(&self) -> String;
    fn from_base58(s: &str) -> BuckyResult<HashValue>;
}

impl HashValueEx for HashValue {
    fn to_base58(&self) -> String {
        self.as_slice().to_base58()
    }

    fn from_base58(s: &str) -> BuckyResult<HashValue> {
        let buf = s.from_base58().map_err(|_e| {
            error!("convert base58 str to HashValue failed, str:{}", s);
            let msg = format!("convert base58 str to object id failed, str={}", s);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if buf.len() != 32 {
            let msg = format!(
                "convert base58 str to object id failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let mut id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), id.as_mut_slice().as_mut_ptr(), buf.len());
        }

        Ok(id)
    }
}
