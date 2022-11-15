use std::sync::Arc;
use async_trait::async_trait;
use cyfs_core::TextObj;
use crate::{Bench, DEC_ID, Stat, util::new_object, DEC_ID2};
use cyfs_base::*;
use cyfs_lib::*;

pub struct SameZoneCryptoBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}

pub const CRYPTO_INNER_ZONE_ACCESS: &str = "crypto-inner-zone-access";
pub const CRYPTO_INNER_ZONE_SIGN: &str = "crypto-inner-zone-sign";
pub const CRYPTO_INNER_ZONE_CODEC: &str = "crypto-inner-zone-codec";
const LIST: [&str;3] = [
    CRYPTO_INNER_ZONE_ACCESS,
    CRYPTO_INNER_ZONE_SIGN,
    CRYPTO_INNER_ZONE_CODEC,
];

#[async_trait]
impl Bench for SameZoneCryptoBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
        
    }

    fn name(&self) -> &str {
        "Same Zone Crypto Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl SameZoneCryptoBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            //self.test_sign(i).await?;
            self.test_crypto(i).await?;

            self.stat.write(self.name(),CRYPTO_INNER_ZONE_ACCESS, begin.elapsed().as_millis() as u64);
        }

        Ok(())
    }

    async fn test_sign(&self, _i: usize) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        // 创建一个随机对象
        let object = new_object("test_sign", "test_crypto");
        let object_raw = object.to_vec().unwrap();
        let id = object.text_id();

        let sign_flags = CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE
            | CRYPTO_REQUEST_FLAG_SIGN_PUSH_DESC
            | CRYPTO_REQUEST_FLAG_SIGN_PUSH_BODY;
        let mut req = CryptoSignObjectRequest::new(id.object_id().to_owned(), object_raw, sign_flags);
        req.common.dec_id = Some(DEC_ID.to_owned());
        req.common.req_path = Some(
            RequestGlobalStatePath::new(Some(DEC_ID.to_owned()), Some("/tests/test_sign".to_owned()))
                .to_string(),
        );

        let resp = self.stack.crypto().sign_object(req).await.unwrap();
        let object_info = resp.object.unwrap();
        assert_eq!(object_info.object_id, *id.object_id());

        // 校验
        let device = self.stack.local_device();
        let sign_object = NONSlimObjectInfo {
            object_id: device.desc().object_id(),
            object_raw: Some(device.to_vec().unwrap()),
            object: None,
        };

        let mut verify_req = CryptoVerifyObjectRequest::new_verify_by_object(
            VerifySignType::Both,
            object_info.clone(),
            sign_object,
        );
        verify_req.common.dec_id = Some(DEC_ID.to_owned());

        let resp = self.stack.crypto().verify_object(verify_req).await.unwrap();
        assert!(resp.result.valid);

        // 错误校验
        let mut verify_req =
            CryptoVerifyObjectRequest::new_verify_by_owner(VerifySignType::Both, object_info);
        verify_req.common.dec_id = Some(DEC_ID.to_owned());

        // 由于object没有owner，所以这里会返回错误
        let resp = self.stack.crypto().verify_object(verify_req).await;
        assert!(resp.is_err());
        self.stat.write(self.name(),CRYPTO_INNER_ZONE_SIGN, begin.elapsed().as_millis() as u64);
        Ok(())
    }

    async fn test_crypto(&self, _i: usize) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        let alphabet: &[u8] = b"0123456789abcdefghijklmnoqprstuvwxyz";

        let system_stack = self.stack
            .fork_with_new_dec(Some(cyfs_core::get_system_dec_app().to_owned()))
            .await.unwrap();
        system_stack.wait_online(None).await.unwrap();
    
        // normal data
        let req = CryptoEncryptDataRequest::new();
        let req = req.by_device().encrypt_data().data(alphabet.to_owned());
        let ret = system_stack.crypto().encrypt_data(req).await.unwrap();
    
        let req = CryptoDecryptDataRequest::new(ret.result);
        let req = req.by_device().decrypt_data();
        let ret = system_stack.crypto().decrypt_data(req).await.unwrap();
        assert_eq!(alphabet, ret.data);
    
        // aes_key
        let req = CryptoEncryptDataRequest::new();
        let req = req.by_device().gen_aeskey_and_encrypt();
        let ret = system_stack.crypto().encrypt_data(req).await.unwrap();
        assert!(ret.aes_key.is_some());
        let aes_key = ret.aes_key.unwrap();
    
        let req = CryptoDecryptDataRequest::new(ret.result);
        let req = req.by_device().decrypt_aeskey();
        let ret = system_stack.crypto().decrypt_data(req).await.unwrap();
    
        assert_eq!(aes_key.as_slice(), ret.data);
        self.stat.write(self.name(),CRYPTO_INNER_ZONE_CODEC, begin.elapsed().as_millis() as u64);
        Ok(())
    }
}