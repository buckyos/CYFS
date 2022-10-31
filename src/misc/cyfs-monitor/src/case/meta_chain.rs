use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, RawFrom, StandardObject, BuckyError, BuckyErrorCode, get_channel, CyfsChannel};
use cyfs_base_meta::{ChainStatus, RequestResult, SavedMetaObject, ViewBalanceResult};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use crate::def::MonitorRunner;

pub struct MetaChainReadMonitor {
    meta_client: MetaClient,
    meta_spv_url: String,
    cur_height: AtomicI64,
    spv_cur_height: AtomicI64
}

pub struct MetaChainWriteMonitor {
    meta_client: MetaClient
}

const PEOPLE_DESC: &str = "00025202002f3ddcc5c8b3b50000000000010030818902818100d0fa2b74489518b1f538da2c7058f05f271b672758bd99fc0b23b4413684ac68fb022810a31ba19c126d9b4a10a45f23bde69defde08be25b0f51d63a71aa186eb802350267037fde804cf2d546aa1cf871e1d9301498f92e9f4b1d12bfbf0f1be3f341d133e2db5490d88b2ae8748831c0827c72b3801ec8e1476fa5dd377650203010001000000000000000000000000000000000000000000000000000000002f3ddcc5c8b3b500010c220a7374616e64616c6f6e65";
const PEOPLE_SEC: &str ="0002623082025e02010002818100d0fa2b74489518b1f538da2c7058f05f271b672758bd99fc0b23b4413684ac68fb022810a31ba19c126d9b4a10a45f23bde69defde08be25b0f51d63a71aa186eb802350267037fde804cf2d546aa1cf871e1d9301498f92e9f4b1d12bfbf0f1be3f341d133e2db5490d88b2ae8748831c0827c72b3801ec8e1476fa5dd3776502030100010281803327297fdb4c73b10bdf90b814001146996201cc05d2d36078b192abebd66a05807bb4a6ede613970a83bde151558adc4addaa874e884153248fbb53eb517f7bffa2960758452c4c83ac8e3f4ad7edbdf21fa1881ac8eb9c08f1fd285b04a9fca15957b99e1a19d63d7b13ad3841313952fcaab27f7cf5a708e3e285f62ff239024100f80b242b494b86bbe1c861b91275a1667cf9bf08773b01445227c3ff55c8ca9c44b85ec0248fa834fc8f22a3530b96b0d81183c3c3fae5fe3190d61f95809a53024100d7ae3a5159ac126a766fb7da6f9c12dc21b3f743b64eecb32f1d6b883c3011dadcfa628b88650d23c0adffcbb23166e7de90d53dce9792c9857adc5951a92067024100a1cf9ad3c627c8084efd4a8ad238fc868066e8315c9e986ffa6c48970c5e459675da14ada1ec395dff985c8f51409118628be27a562219e19e720ecd61d91853024100bdd70043dc8f25c289aefa000d9f2dc68eefce23ce9317aafc3c840aed174e8ffb53746be6c9335095e751b0a48ef14a04502d31f2e6dd6ffdca4fab5ac267df024100947221ea28e587c8367a0250993b6beba82fd6286e34ae531251482eb6312b1e5b03d0851d471743b552d9c8a8a4436416648c2a6b662aeb2f26d53a98741cb0";

impl MetaChainReadMonitor {
    pub(crate) fn new() -> Self {
        let meta_spv_url = match get_channel() {
            CyfsChannel::Nightly => {
                "http://154.31.50.111:3516"
            }
            CyfsChannel::Beta => {
                "http://106.75.152.253:1563"
            }
            CyfsChannel::Stable => {
                ""
            }
        }.to_owned();

        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            meta_spv_url,
            cur_height: AtomicI64::new(0),
            spv_cur_height: AtomicI64::new(0)
        }
    }
}

impl MetaChainWriteMonitor {
    pub(crate) fn new() -> Self {
        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::default())
        }
    }
}

#[async_trait::async_trait]
impl MonitorRunner for MetaChainReadMonitor {
    fn name(&self) -> &str {
        "meta_chain_read"
    }

    async fn run_once(&self, _: bool) -> BuckyResult<()> {
        // 测试从链上读取
        let object_id = cyfs_base::People::clone_from_hex(PEOPLE_DESC, &mut vec![])?.desc().calculate_id();
        let balance = self.meta_client.get_balance(&object_id, 0).await?;
        match balance {
            ViewBalanceResult::Single(balance) => {
                if balance[0].1 < 1000 {
                    return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, format!("account {} balance {} less then 1000", &object_id, balance[0].1)));
                }
            }
            ViewBalanceResult::Union(_) => {
                return Err(BuckyError::new(BuckyErrorCode::NotMatch, format!("account {} return balance type mismatch!", &object_id)));
            }
        }

        // 检查当前高度
        let status = self.meta_client.get_chain_status().await?;
        let cur_height = self.cur_height.load(Ordering::SeqCst);
        if cur_height > 0 && status.height == cur_height {
            return Err(BuckyError::new(BuckyErrorCode::Timeout, format!("chain height stop at {}, chain hang?", status.height)));
        }
        self.cur_height.store(status.height, Ordering::SeqCst);

        // 检查spv高度
        let spv_status: RequestResult<ChainStatus> = surf::get(format!("{}/status", &self.meta_spv_url)).recv_json().await?;
        if spv_status.err != 0 {
            return Err(BuckyError::new(spv_status.err, spv_status.msg));
        } else {
            let spv_cur_height = self.spv_cur_height.load(Ordering::SeqCst);
            let spv_height = spv_status.result.unwrap().height;
            if spv_cur_height > 0 && spv_height == spv_cur_height {
                return Err(BuckyError::new(BuckyErrorCode::Timeout, format!("spv height stop at {}, spv hang?", spv_height)));
            }
            self.spv_cur_height.store(spv_height, Ordering::SeqCst);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl MonitorRunner for MetaChainWriteMonitor {
    fn name(&self) -> &str {
        "meta_chain_write"
    }

    async fn run_once(&self, _: bool) -> BuckyResult<()> {
        // 测试往链上写入
        let mut people = cyfs_base::People::clone_from_hex(PEOPLE_DESC, &mut vec![])?;
        let people_id = people.desc().calculate_id();
        let secret = cyfs_base::PrivateKey::clone_from_hex(PEOPLE_SEC, &mut vec![])?;
        let update_time = cyfs_base::bucky_time_now();
        people.body_mut_expect("").set_update_time(update_time);
        let data = SavedMetaObject::People(people.clone());
        let txid = self.meta_client.update_desc(&StandardObject::People(people), &data, None, None, &secret).await.map_err(|e|{
            error!("update desc err {}", e);
            e
        })?;
        let mut success = false;
        for _ in 0..3 {
            if let Ok(Some((ret, _))) = self.meta_client.get_tx_receipt(&txid).await {
                if ret.result != cyfs_base_meta::ERROR_SUCCESS as u32 {
                    let msg = format!("tx {} execute result {}", &txid, ret.result);
                    return Err(BuckyError::new(BuckyErrorCode::MetaError(ret.result as u16), msg));
                }
                success = true;
                break;
            }

            async_std::task::sleep(Duration::from_secs(10)).await;
        }

        if !success {
            let msg = format!("tx {} execute no result", &txid);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }
        // 测试写入是否正确
        let resp_desc = self.meta_client.get_desc(&people_id).await?;
        match resp_desc {
            SavedMetaObject::People(p) => {
                if p.body_expect("").update_time() != update_time {
                    let msg = format!("get {} update time mismatch, except {}, actual {}", &people_id, update_time, p.body_expect("").update_time());
                    return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
                }
            }
            _ => {
                let msg = format!("get {} type error", &people_id);
                return Err(BuckyError::new(BuckyErrorCode::NotMatch, msg));
            }
        }

        Ok(())
    }
}