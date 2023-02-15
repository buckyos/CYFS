use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use cyfs_base::{BuckyResult, NamedObject, ObjectDesc, StandardObject, BuckyError, BuckyErrorCode, get_channel, CyfsChannel, PrivateKey, People, FileEncoder, FileDecoder, ObjectId};
use cyfs_base_meta::{ChainStatus, RequestResult, SavedMetaObject, ViewBalanceResult};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_util::{get_service_data_dir};
use crate::def::MonitorRunner;
use crate::SERVICE_NAME;

pub struct MetaChainReadMonitor {
    meta_client: MetaClient,
    meta_spv_url: String,
    cur_height: AtomicI64,
    spv_cur_height: AtomicI64,
    desc_id: ObjectId
}

pub struct MetaChainWriteMonitor {
    meta_client: MetaClient,
    sec: PrivateKey,
    desc: People
}

impl MetaChainReadMonitor {
    pub(crate) fn new() -> Self {
        let meta_spv_url = match get_channel() {
            CyfsChannel::Nightly => {
                "https://nightly.meta.cyfs.com:13516"
            }
            CyfsChannel::Beta => {
                "https://beta.meta.cyfs.com:13516"
            }
            CyfsChannel::Stable => {
                ""
            }
        }.to_owned();

        let (_, people) = ensure_desc().unwrap();

        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            meta_spv_url,
            cur_height: AtomicI64::new(0),
            spv_cur_height: AtomicI64::new(0),
            desc_id: people.desc().calculate_id()
        }
    }
}

fn ensure_desc() -> BuckyResult<(PrivateKey, People)> {
    let path = get_service_data_dir(SERVICE_NAME).join("meta-writer").join("desc");
    let desc_path = path.with_extension("desc");
    let sec_path = path.with_extension("sec");
    if !desc_path.exists() || !sec_path.exists()  {
        info!("not found meta writer desc, create.");
        let pk = PrivateKey::generate_rsa(1024)?;
        let people = People::new(None, vec![], pk.public(), None, None, None).build();
        pk.encode_to_file(&sec_path, false)?;
        people.encode_to_file(&desc_path, false)?;
        Ok((pk, people))
    } else {
        let (pk, _) = PrivateKey::decode_from_file(&sec_path, &mut vec![])?;
        let (people, _) = People::decode_from_file(&desc_path, &mut vec![])?;
        Ok((pk, people))
    }
}

impl MetaChainWriteMonitor {
    pub(crate) fn new() -> Self {
        let (pk, people) = ensure_desc().unwrap();
        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            sec: pk,
            desc: people,
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
        let balance = self.meta_client.get_balance(&self.desc_id, 0).await?;
        match balance {
            ViewBalanceResult::Single(balance) => {
                if balance[0].1 < 1000 {
                    return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, format!("account {} balance {} less then 1000", &self.desc_id, balance[0].1)));
                }
            }
            ViewBalanceResult::Union(_) => {
                return Err(BuckyError::new(BuckyErrorCode::NotMatch, format!("account {} return balance type mismatch!", &self.desc_id)));
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
        let mut people = self.desc.clone();
        let people_id = people.desc().calculate_id();
        let update_time = cyfs_base::bucky_time_now();
        people.body_mut_expect("").set_update_time(update_time);
        let data = SavedMetaObject::People(people.clone());
        let txid = self.meta_client.update_desc(&StandardObject::People(people), &data, None, None, &self.sec).await.map_err(|e|{
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