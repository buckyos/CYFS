use crate::chain::{BaseMiner, BFTPrepareRequest, BFTPrepareResponse, BFTProto, BFTProtoDescContent, new_bft_proto, BlockExecutor, MinerRuner, BFTChangeView, BFTNodeSync, BFTNodeSyncResponse, BFTError};
use crate::{Miner, Chain};
use cyfs_base::*;
use crate::network::{ChainNetwork};
use std::path::Path;
use crate::state_storage::StorageRef;
use cyfs_base_meta::{Block, BlockDescTrait};
use async_trait::async_trait;
use log::*;
use std::str::FromStr;
use std::sync::{Arc, mpsc, Mutex, MutexGuard};
use std::sync::mpsc::{Sender, Receiver};
use std::thread::JoinHandle;
use timer::Timer;
use crate::helper::{get_meta_err_code, ArcWeakHelper};
use crate::*;
use cyfs_core::*;
use std::collections::HashMap;
use std::time::Duration;
use crate::mint::btc_mint::BTCMint;
use crate::mint::subchain_mint::SubChainMint;
use crate::executor::context::Config;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum BFTMinerStatus {
    None,
    Init,
    WaitingCreate,
    WaitingProposal,
    WaitingAgree,
    ChangeViewSent,
    ChangeViewSuccess,
}

impl BFTMinerStatus {
    fn to_string(&self) -> &'static str {
        match self {
            BFTMinerStatus::None => "None",
            BFTMinerStatus::Init => "Init",
            BFTMinerStatus::WaitingCreate => "WaitingCreate",
            BFTMinerStatus::WaitingProposal => "WaitingProposal",
            BFTMinerStatus::WaitingAgree => "WaitingAgree",
            BFTMinerStatus::ChangeViewSent => "ChangeViewSent",
            BFTMinerStatus::ChangeViewSuccess => "ChangeViewSuccess",
        }
    }
}

#[derive(Eq, PartialEq, Copy, Clone)]
enum TimerAction {
    CreateBlock,
    ChangeView(u8),
}

enum BFTMinerMsg {
    StartMineBlock,
    Exit,
    Timeout(TimerAction),
    PrepareRequest(BFTPrepareRequest, Block),
    PrepareResponse(BFTPrepareResponse),
    ChangeView(BFTChangeView),
}

pub struct BFTMinerStatusInfo {
    pub status: BFTMinerStatus,
    pub status_change_time: u64,
    pub view: u8,
    pub change_view_list: Vec<BFTChangeView>,
    pub prepare_response_list: Vec<BFTPrepareResponse>,
    pub speaker: Option<u8>,
    pub speaker_desc: Option<DeviceDesc>,
    pub height: i64,
    pub block: Option<Block>,
}

impl BFTMinerStatusInfo {
    fn new() -> Self {
        BFTMinerStatusInfo {
            status: BFTMinerStatus::None,
            status_change_time: bucky_time_now(),
            view: 0,
            change_view_list: Vec::new(),
            prepare_response_list: Vec::new(),
            speaker: None,
            speaker_desc: None,
            height: 0,
            block: None,
        }
    }

    fn clear(&mut self) {
        self.status = BFTMinerStatus::Init;
        self.status_change_time = bucky_time_now();
        self.view = 0;
        self.change_view_list = Vec::new();
        self.prepare_response_list = Vec::new();
        self.speaker = None;
        self.speaker_desc = None;
        self.height = 0;
        self.block = None;
    }

    fn change_status(&mut self, new_status: BFTMinerStatus) {
        self.status = new_status;
        self.status_change_time = bucky_time_now();
    }
}

pub struct BFTMiner<NETWORK: 'static + ChainNetwork> {
    chain_type: String,
    base: BaseMiner,
    network: NETWORK,
    miners: Mutex<Vec<DeviceDesc>>,
    miner_key: PrivateKey,
    sender: Mutex<Sender<BFTMinerMsg>>,
    receiver: Mutex<Option<Receiver<BFTMinerMsg>>>,
    mining_thread: Mutex<Option<JoinHandle<()>>>,
    time_executor: Mutex<Timer>,
    timer: Mutex<Option<timer::Guard>>,
    status_info: Mutex<BFTMinerStatusInfo>,
}

impl<NETWORK: 'static + ChainNetwork> BFTMiner<NETWORK> {
    pub fn new(chain_type: String,
               coinbase: ObjectId,
               interval: u32,
               chain: Chain,
               bfc_spv_node: String,
               network: NETWORK,
               miner_key: PrivateKey) -> BuckyResult<Self> {

        let (sender, receiver) = mpsc::channel();
        Ok(BFTMiner {
            chain_type,
            base: BaseMiner::new(coinbase, interval, chain, bfc_spv_node, Some(miner_key.clone())),
            network,
            miners: Mutex::new(Vec::new()),
            miner_key,
            sender: Mutex::new(sender),
            receiver: Mutex::new(Some(receiver)),
            mining_thread: Mutex::new(None),
            time_executor: Mutex::new(Timer::new()),
            timer: Mutex::new(None),
            status_info: Mutex::new(BFTMinerStatusInfo::new()),
        })
    }

    pub async fn load(chain_type: String,
                      coinbase: ObjectId,
                      interval: u32,
                      bfc_spv_node: String,
                      dir: &Path, 
                      new_storage: fn(path: &Path) -> StorageRef,
                      trace: bool,
                      archive_storage: fn(path: &Path, trace: bool) -> ArchiveStorageRef,
                      network: NETWORK,
                      miner_key: PrivateKey) -> BuckyResult<Self> {
        let chain = Chain::load(dir, new_storage, trace, archive_storage).await?;
        let (sender, receiver) = mpsc::channel();
        Ok(BFTMiner {
            chain_type,
            base: BaseMiner::new(coinbase, interval, chain, bfc_spv_node, Some(miner_key.clone())),
            network,
            miners: Mutex::new(Vec::new()),
            miner_key,
            sender: Mutex::new(sender),
            receiver: Mutex::new(Some(receiver)),
            mining_thread: Mutex::new(None),
            time_executor: Mutex::new(Timer::new()),
            timer: Mutex::new(None),
            status_info: Mutex::new(BFTMinerStatusInfo::new()),
        })
    }

    async fn self_index(&self) -> BuckyResult<u8> {
        let miners = self.get_miners().await?;
        let mut index: u8 = 0;
        for miner in miners.iter() {
            if &miner.calculate_id() ==  self.base.coinbase() {
                break;
            }
            index += 1;
        }
        Ok(index)
    }

    async fn self_miner(&self) -> BuckyResult<Option<DeviceDesc>> {
        let miners = self.get_miners().await?;
        for miner in miners.iter() {
            if &miner.calculate_id() ==  self.base.coinbase() {
                return Ok(Some(miner.clone()))
            }
        }
        Ok(None)
    }

    async fn is_miners(&self, id: &ObjectId) -> BuckyResult<bool> {
        let miners = self.get_miners().await?;
        let mut find = false;
        for miner in miners.iter() {
            if &miner.calculate_id() ==  id {
                find = true;
                break;
            }
        }
        Ok(find)
    }

    async fn on_recv_tx(&self, tx: &MetaTx) -> BuckyResult<Vec<u8>> {
        info!("recv tx:{}", tx.desc().calculate_id().to_string());
        self.base.push_tx(tx.clone()).await?;
        Ok(Vec::new())
    }

    async fn on_recv_get_block(&self, number: i64) -> BuckyResult<Vec<u8>> {
        info!("recv get block:{}", number);
        let block = self.base.as_chain().get_chain_storage().get_block_by_number(number).await?;

        Ok(block.to_vec()?)
    }

    async fn verify_prepare_block_sign(&self, block: &Block) -> BuckyResult<bool> {
        let public_key = {
            let status_info = self.status_info.lock().unwrap();
            status_info.speaker_desc.as_ref().unwrap().public_key().clone()
        };
        let desc_signs_opt = block.signs().desc_signs();
        if desc_signs_opt.is_none() {
            log::error!("desc signs is none");
            return Ok(false);
        }
        let desc_signs = desc_signs_opt.unwrap();
        if desc_signs.len() == 0 {
            log::error!("desc signs len is 0");
            return Ok(false);
        }

        let signature = &desc_signs[0];
        let verifier = RsaCPUObjectVerifier::new(public_key);
        verify_object_desc_sign(&verifier, block, signature).await
    }

    async fn on_recv_prepare_request(&self, request: BFTPrepareRequest, mut block: Block) -> BuckyResult<()> {
        log::info!("on_recv_prepare_request cur {} height {} view {} speaker {} hash {}",
                   self.self_index().await?,
                   block.desc().number(),
                   request.view,
                   request.speaker,
                   request.block_id.to_string());
        {
            let mut status_info = self.status_info.lock().unwrap();
            if status_info.status != BFTMinerStatus::WaitingProposal && status_info.status != BFTMinerStatus::ChangeViewSent {
                if !(status_info.status == BFTMinerStatus::WaitingAgree
                    && status_info.speaker.unwrap() == self.self_index().await?
                    && bucky_time_now() - status_info.status_change_time > self.base.interval() as u64 * 1000000) {
                    log::info!("status {} expect WaitingProposal", status_info.status.to_string());
                    return Ok(());
                }
            }

            let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
            if tip_desc.number() < block.desc().number() - 1 {
                let node = {
                    let miners = self.get_miners().await?;
                    let miner: Option<&DeviceDesc> = miners.get(request.speaker as usize);
                    if miner.is_some() {
                        self.network.get_node(miner.unwrap().calculate_id().to_string().as_str())
                    } else {
                        None
                    }
                };
                let ret = self.sync_chain(node.clone()).await;
                if ret.is_ok() {
                    let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
                    status_info.height = tip_desc.number() + 1;
                    if status_info.height == block.desc().number() {
                        status_info.view = request.view;
                        let (index, speaker_desc) = self.get_next_block_miner(&tip_desc, request.view as u32).await?;
                        status_info.speaker = Some(index as u8);
                        status_info.speaker_desc = Some(speaker_desc);
                        status_info.change_view_list.clear();
                        status_info.prepare_response_list.clear();
                        status_info.block = None;
                        log::info!("cur {} change status from {} to WaitingProposal", self.self_index().await?, status_info.status.to_string());
                        status_info.change_status(BFTMinerStatus::WaitingProposal);
                    } else {
                        log::info!("cur {} change status from {} to None", self.self_index().await?, status_info.status.to_string());
                        status_info.clear();
                        self.stop_timer();
                        self.sender.lock().unwrap().send(BFTMinerMsg::StartMineBlock).unwrap();
                        return Ok(());
                    }
                } else {
                    if node.is_some() {
                        log::info!("cur {} sync chain from {} failed.err {:?}", self.self_index().await?, node.unwrap(), ret.err().unwrap());
                    } else {
                        log::info!("cur {} sync chain failed.err {:?}", self.self_index().await?, ret.err().unwrap());
                    }
                }
            }

            if block.desc().number() != status_info.height
                || request.view != status_info.view
                || block.desc().coinbase() != &status_info.speaker_desc.as_ref().unwrap().calculate_id() {
                log::info!("height {} view {} owner {} expect height {} view {} owner {}",
                           block.desc().number(),
                           request.view,
                           block.desc().coinbase(),
                           status_info.height,
                           status_info.view,
                           status_info.speaker_desc.as_ref().unwrap().calculate_id());
                return Ok(());
            }

        }


        if self.verify_prepare_block_sign(&block).await? {
            let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
            if &tip_desc.hash() != block.desc().pre_block_hash() || tip_desc.number() + 1 != block.desc().number() {
                log::error!("pre block hash check err.local {} height {} recv {} height {}",
                            tip_desc.hash().to_string(),
                            tip_desc.number(),
                            block.desc().pre_block_hash().to_string(),
                            block.desc().number());
                return Err(meta_err!(ERROR_BLOCK_VERIFY_FAILED));
            }
            let storage = self.base.as_chain().get_chain_storage().state_storage();
            let _ = storage.recovery(tip_desc.number()).await;
            // state_ref.being_transaction().await?;

            let archive_storage = self.base.as_chain().get_chain_storage().archive_storage();

            log::info!("thread {:?} cur {} verify block {} {} ",
                       std::thread::current().id(),
                       self.self_index().await?,
                       block.desc().number(),
                       block.desc().calculate_id().to_string());
            let ret = BlockExecutor::execute_and_verify_block(&block,
                                                              &storage,
                                                              &archive_storage,
                                                              Some(self.base.as_chain().get_chain_storage()),
                                                              self.base.bfc_spv_node(),
                                                              Some(self.miner_key.clone()),
                                                              self.base.coinbase().clone()).await?;
            if !ret {
                // state_ref.rollback().await?;
                log::error!("block {} verify failed", block.desc().number());
                return Err(meta_err!(ERROR_BLOCK_VERIFY_FAILED));
            }
            // state_ref.commit().await?;
        } else {
            log::error!("block {} signature verify failed", block.desc().number());
            return Err(meta_err!(ERROR_SIGNATURE_ERROR));
        }

        let member = self.self_index().await?;
        let sign = block.sign(self.miner_key.clone(), &SignatureSource::RefIndex(member)).await?;

        let req = BFTPrepareResponse {
            height: block.desc().number(),
            view: request.view,
            member,
            sign
        };

        let proto = self.new_proto_obj(BFTProtoDescContent::PrepareResponse(req), Vec::new()).await?;
        log::info!("broadcast PrepareResponse begin");
        self.network.broadcast(proto.to_vec()?).await?;
        log::info!("broadcast PrepareResponse end");

        let prepare_response_list = {
            let mut status_info = self.status_info.lock().unwrap();
            log::info!("cur {} change status from {} to WaitingAgree", self.self_index().await?, status_info.status.to_string());
            status_info.change_status(BFTMinerStatus::WaitingAgree);
            status_info.block = Some(block);

            let prepare_response_list = status_info.prepare_response_list.clone();
            status_info.prepare_response_list.clear();
            prepare_response_list
        };

        for response in prepare_response_list {
            self.on_recv_prepare_response(response).await?;
        }

        Ok(())
    }

    async fn on_recv_prepare_response(&self, response: BFTPrepareResponse) -> BuckyResult<()> {
        log::info!("on_recv_prepare_response:cur {} height {} view {} member {}",
                   self.self_index().await?,
                   response.height,
                   response.view,
                   response.member);
        {
            let mut status_info = self.status_info.lock().unwrap();
            if status_info.height != response.height
                || status_info.view != response.view {
                log::error!("height {} view {} expect height {} view {}", status_info.height, status_info.view, response.height, response.view);
                return Ok(());
            }

            let mut find = false;
            for has_resp in &status_info.prepare_response_list {
                if has_resp.member == response.member {
                    find = true;
                    break;
                }
            }
            if find {
                log::info!("height {} view {} member {} has exist", response.height, response.view, response.member);
                return Ok(());
            }

            if status_info.status != BFTMinerStatus::WaitingAgree && status_info.status != BFTMinerStatus::ChangeViewSent {
                log::info!("status {} expect WaitingAgree or ChangeViewSent", status_info.status.to_string());
                status_info.prepare_response_list.push(response);
                return Ok(());
            }

            if status_info.block.is_none() {
                return Ok(());
            }

            let device_desc: DeviceDesc = {
                let miners = self.get_miners().await?;
                miners.get(response.member as usize).unwrap().clone()
            };

            let verifier = RsaCPUObjectVerifier::new(device_desc.public_key().clone());
            let verify = verify_object_desc_sign(&verifier, status_info.block.as_ref().unwrap(), &response.sign).await?;
            if !verify {
                log::info!("verify sign failed");
                return Ok(());
            }

            status_info.prepare_response_list.push(response);

            let len = {
                let miners = self.get_miners().await?;
                miners.len()
            };

            let self_index = self.self_index().await?;
            if (status_info.speaker.unwrap() == self_index && status_info.prepare_response_list.len() >= (0.7 * len as f32).ceil() as usize - 1)
            || (status_info.speaker.unwrap() != self_index && status_info.prepare_response_list.len() >= (0.7 * len as f32).ceil() as usize - 2){
                let mut block = status_info.block.take().unwrap();
                for resp in &status_info.prepare_response_list {
                    block.signs_mut().push_desc_sign(resp.sign.clone());
                }

                for tx in block.transactions() {
                    self.base.remove(tx).await?;
                }
                self.base.as_chain().add_mined_block(&block).await?;
                let _ = self.base.as_chain().backup(block.desc().number()).await;

                log::info!("cur {} mined block {} change status from {} to None", self.self_index().await?, block.desc().number(), status_info.status.to_string());
                status_info.clear();
                self.stop_timer();
                self.sender.lock().unwrap().send(BFTMinerMsg::StartMineBlock).unwrap();
            }
        }


        Ok(())
    }

    async fn check_change_view_reqs(&self) -> BuckyResult<()> {
        let len = {
            let miners = self.get_miners().await?;
            miners.len()
        };
        let mut status_info = self.status_info.lock().unwrap();

        let self_index = self.self_index().await?;
        if (status_info.status == BFTMinerStatus::ChangeViewSent && status_info.change_view_list.len() >= (0.7 * len as f32).ceil() as usize - 2)//非出块节点必须已经发送changeview事件
            || (status_info.status == BFTMinerStatus::WaitingAgree
            && status_info.speaker.unwrap() == self_index
            && status_info.change_view_list.len() >= (0.7 * len as f32).ceil() as usize - 1) {  //出块节点才能在WaitingAgree状态切换View
            let mut change_map = HashMap::<String, i32>::new();
            for i in 1..status_info.view+2 {
                let key = format!("{}_{}", status_info.height, i);
                change_map.insert(key, 1);
            }
            for view_req in &status_info.change_view_list {
                let dest_view = view_req.dest_view;
                let height = view_req.height;
                let key = format!("{}_{}", view_req.height, dest_view);
                if change_map.contains_key(key.as_str()) {
                    let mut count = change_map.get(key.as_str()).unwrap().clone();
                    count += 1;
                    if count >= (0.7 * len as f32).ceil() as i32 - 1 {
                        status_info.change_view_list.clear();
                        status_info.prepare_response_list.clear();
                        status_info.block = None;
                        status_info.height = height;
                        log::info!("cur {} change status from {} to ChangeViewSuccess", self.self_index().await?, status_info.status.to_string());
                        status_info.change_status(BFTMinerStatus::ChangeViewSuccess);
                        status_info.view = dest_view;
                        self.stop_timer();

                        self.sender.lock().unwrap().send(BFTMinerMsg::StartMineBlock).unwrap();
                        break;
                    } else {
                        change_map.insert(key, count);
                    }
                } else {
                    change_map.insert(key, 1);
                }
            }
        }
        Ok(())
    }

    async fn on_recv_change_view(&self, change_view: BFTChangeView) -> BuckyResult<()> {
        {
            let self_index = self.self_index().await?;
            log::info!("on_recv_change_view cur {} height {} view {} dest_view {} member {}",
                     self_index,
                     change_view.height,
                     change_view.view,
                     change_view.dest_view,
                     change_view.member);
            let mut status_info = self.status_info.lock().unwrap();

            //如果当前节点块高度小于对方，则同步块
            if status_info.height > 0 && status_info.height < change_view.height {
                let node = {
                    let miners = self.get_miners().await?;
                    let miner: Option<&DeviceDesc> = miners.get(change_view.member as usize);
                    if miner.is_some() {
                        self.network.get_node(miner.unwrap().calculate_id().to_string().as_str())
                    } else {
                        None
                    }
                };
                let ret = self.sync_chain(node.clone()).await;
                if ret.is_ok() {
                    let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
                    status_info.height = tip_desc.number() + 1;
                    status_info.view = change_view.view;

                    let (index, speaker_desc) = self.get_next_block_miner(&tip_desc, status_info.view as u32).await?;
                    status_info.speaker = Some(index as u8);
                    status_info.speaker_desc = Some(speaker_desc);
                    status_info.change_view_list.clear();
                    status_info.prepare_response_list.clear();
                    status_info.block = None;

                    log::info!("cur {} change status from {} to WaitingProposal", self.self_index().await?, status_info.status.to_string());
                    status_info.change_status(BFTMinerStatus::WaitingProposal);
                    self.stop_timer();
                    self.reset_timer(TimerAction::ChangeView(change_view.view), 0);
                } else {
                    if node.is_some() {
                        log::info!("cur {} sync chain from {} failed.err {:?}", self.self_index().await?, node.unwrap(), ret.err().unwrap());
                    } else {
                        log::info!("cur {} sync chain failed.err {:?}", self.self_index().await?, ret.err().unwrap());
                    }
                }
            }

            if status_info.height != change_view.height {
                log::info!("change height {} is not equal expect {}.ignore", change_view.height, status_info.height);
                return Ok(());
            }

            let mut find = false;
            for has_view in &status_info.change_view_list {
                if change_view.height == has_view.height
                    && change_view.view == has_view.view
                    && change_view.dest_view == has_view.dest_view
                    && change_view.member == has_view.member {
                    find = true;
                }
            }

            if find {
                log::error!("request has exist. height {} view {} dest_view {} member {}",
                            change_view.height,
                change_view.view,
                change_view.dest_view,
                change_view.member);
                return Ok(())
            }

            status_info.change_view_list.push(change_view);
        }

        self.check_change_view_reqs().await
    }

    async fn on_recv_get_height(&self) -> BuckyResult<Vec<u8>> {
        let (block_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
        let height = block_desc.number();
        let resp_obj = self.new_proto_obj(BFTProtoDescContent::GetHeightResp(height), Vec::new()).await?;
        resp_obj.to_vec()
    }

    async fn on_recv_node_sync(&self, req: &BFTNodeSync) -> BuckyResult<Vec<u8>> {
        self.network.add_node(req.node_id.as_str(), req.addr.as_str()).await?;
        let node_list = self.network.get_node_list()?;

        let status = {
            let status_info = self.status_info.lock().unwrap();
            status_info.status
        };
        if status == BFTMinerStatus::None && self.is_network_valid().await? {
            let status = {
                let status_info = self.status_info.lock().unwrap();
                status_info.status
            };

            log::info!("cur {} status from {} to Init", self.self_index().await?, status.to_string());
            let mut status_info = self.status_info.lock().unwrap();
            status_info.change_status(BFTMinerStatus::Init);
            self.sender.lock().unwrap().send(BFTMinerMsg::StartMineBlock).unwrap();
        }

        let sync_resp = BFTNodeSyncResponse {
            node_id: self.base.coinbase().to_string(),
            addr_list: node_list
        };

        let resp_obj = self.new_proto_obj(BFTProtoDescContent::NodeSyncResponse(sync_resp), Vec::new()).await?;
        resp_obj.to_vec()


    }

    async fn new_proto_obj(&self, proto: BFTProtoDescContent, proto_data: Vec<u8>) -> BuckyResult<BFTProto> {
        let mut proto = new_bft_proto(self.base.coinbase().clone(), proto, proto_data).build();

        let signer = RsaCPUObjectSigner::new(self.miner_key.public(), self.miner_key.clone());
        sign_and_set_named_object_desc(&signer, &mut proto, &SignatureSource::Key(PublicKeyValue::Single(self.miner_key.public()))).await?;
        Ok(proto)
    }

    async fn verify_proto_obj(&self, proto: &BFTProto, verify_owner: bool) -> BuckyResult<bool> {
        let owner_opt = proto.desc().owner();
        if owner_opt.is_none() {
            return Ok(false);
        }
        let owner = owner_opt.unwrap();
        if verify_owner && !self.is_miners(&owner).await? {
            return Ok(false);
        }

        let desc_signs = proto.signs().desc_signs();
        if desc_signs.is_none() {
            return Ok(false);
        }

        let signs = desc_signs.unwrap();
        if signs.len() == 0 {
            return Ok(false);
        }
        let sign = signs.get(0).unwrap();
        match sign.sign_source() {
            SignatureSource::Key(PublicKeyValue::Single(public_key)) => {
                let verifier = RsaCPUObjectVerifier::new(public_key.clone());
                verify_object_desc_sign(&verifier, proto, sign).await
            }
            SignatureSource::RefIndex(0) => {
                let state_storage = self.base.as_chain().get_chain_storage().state_storage();
                let state_ref = state_storage.create_state(true).await;
                let desc = state_ref.get_obj_desc(&owner).await?;
                if let SavedMetaObject::Device(device) = desc {
                    let public_key = device.desc().public_key();
                    let verifier = RsaCPUObjectVerifier::new(public_key.clone());
                    verify_object_desc_sign(&verifier, proto, sign).await
                } else {
                    Ok(false)
                }
            }
            _ => {
                Ok(false)
            }
        }

    }

    async fn get_miners(&self) -> BuckyResult<MutexGuard<'_, Vec<DeviceDesc>>> {
        {
            let miners = self.miners.lock().unwrap();
            if miners.len() != 0 {
                return Ok(miners);
            }
        }

        let state_storage = self.base.as_chain().get_chain_storage().state_storage();
        let state_ref = state_storage.create_state(true).await;
        let org_id = state_ref.config_get("miners_group", "").await?;
        if org_id != "" {
            let miners = self.miners.lock().unwrap();
            if miners.len() != 0 {
                return Ok(miners);
            }
        }


        let saved_org = state_ref.get_account_info(&ObjectId::from_str(org_id.as_str())?).await?;
        if let AccountInfo::MinerGroup(miner_group) = saved_org {
            let mut miners = self.miners.lock().unwrap();
            *miners = miner_group.members().clone();
            Ok(miners)
        } else {
            log::error!("miners type error");
            Err(meta_err!(ERROR_EXCEPTION))
        }
    }

    async fn get_next_block_miner(&self, cur_block: &BlockDesc, view: u32) -> BuckyResult<(usize, DeviceDesc)> {
        assert_eq!(std::thread::current().id(), self.mining_thread.lock().unwrap().as_ref().unwrap().thread().id());
        let miners = self.get_miners().await?;
        if miners.len() == 0 {
            return Err(meta_err!(ERROR_NONE_MINERS));
        }
        let mut i = 0;
        for miner in miners.iter() {
            if &miner.calculate_id() == cur_block.coinbase() {
                break;
            }
            i += 1;
        }
        let index = (i + 1 + view) as usize % miners.len();
        // println!("get_next_block_miner view {} miner {}", view, index);
        Ok((index, miners.get(index).unwrap().clone()))
    }

    async fn verify_block_sign(&self, block: &Block) -> BuckyResult<bool> {
        if block.desc().number() == 0 {
            let desc_signs_opt = block.signs().desc_signs();
            if desc_signs_opt.is_none() {
                log::error!("desc signs is none");
                return Ok(false);
            }
            let desc_signs = desc_signs_opt.unwrap();
            if desc_signs.len() == 0 {
                log::error!("desc signs len is 0");
                return Ok(false);
            }

            let signature = &desc_signs[0];
            let sign_source = signature.sign_source();
            if let SignatureSource::Key(PublicKeyValue::Single(public_key)) = sign_source {
                let verifier = RsaCPUObjectVerifier::new(public_key.clone());
                verify_object_desc_sign(&verifier, block, &signature).await
            } else {
                log::error!("desc signs verify failed");
                Ok(false)
            }
        } else {
            let miners = self.get_miners().await?;
            let desc_signs_opt = block.signs().desc_signs();
            if desc_signs_opt.is_none() {
                log::error!("desc signs is none");
                return Ok(false);
            }
            let desc_signs = desc_signs_opt.unwrap();
            if desc_signs.len() < (0.7 * miners.len() as f32).ceil() as usize || desc_signs.len() > miners.len() {
                log::error!("desc signs is valid");
                return Ok(false);
            }

            for sign in desc_signs {
                if let SignatureSource::RefIndex(i) = sign.sign_source() {
                    let device: &DeviceDesc = miners.get(*i as usize).unwrap();
                    let public_key = device.public_key();
                    let verifier = RsaCPUObjectVerifier::new(public_key.clone());
                    if !verify_object_desc_sign(&verifier, block, &sign).await? {
                        log::error!("desc signs verify failed");
                        return Ok(false);
                    }
                } else {
                    log::error!("desc signs verify failed");
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }

    async fn request(&self, param: Vec<u8>, to: Option<String>) -> BuckyResult<Vec<u8>> {
        if to.is_none() {
            let data;
            loop {
                let ret = self.network.request(param.clone(), None).await;
                if ret.is_ok() {
                    data = ret.unwrap();
                    break;
                } else {
                    log::error!("err {}", ret.err().unwrap());
                    async_std::task::sleep(Duration::new(5, 0)).await;
                }
            }
            Ok(data)
        } else {
            self.network.request(param.clone(), None).await
        }
    }
    async fn get_chain_height(&self, to: Option<String>) -> BuckyResult<i64> {
        let param = self.new_proto_obj(BFTProtoDescContent::GetHeight, Vec::new()).await?;
        let resp = self.request(param.to_vec()?, to).await?;
        let proto = BFTProto::clone_from_slice(resp.as_slice())?;
        let ret = self.verify_proto_obj(&proto, false).await?;
        if !ret {
            log::error!("get_chain_height obj sign verify failed");
            return Err(meta_err!(ERROR_SIGNATURE_ERROR));
        }

        let desc_content = proto.desc().content();
        if let BFTProtoDescContent::GetHeightResp(height) = desc_content {
            Ok(*height)
        } else {
            log::error!("get_chain_height obj type error");
            Err(meta_err!(ERROR_INVALID))
        }
    }

    async fn get_block(&self, number: i64, to: Option<String>) -> BuckyResult<Block> {
        let param = self.new_proto_obj(BFTProtoDescContent::GetBlock(number), Vec::new()).await?;
        let resp = self.request(param.to_vec()?, to).await?;
        let block = Block::clone_from_slice(resp.as_slice())?;
        let ret = self.verify_block_sign(&block).await?;
        if !ret {
            log::error!("get_block obj sign verify failed");
            return Err(meta_err!(ERROR_SIGNATURE_ERROR));
        }
        Ok(block)
    }

    async fn on_recv(&self, data: Vec<u8>) -> BuckyResult<Vec<u8>> {
        let (obj, _) = AnyNamedObject::raw_decode(data.as_slice())?;
        if ObjectTypeCode::Custom == obj.obj_type_code() {
            if obj.obj_type() == CoreObjectType::MetaProto as u16 {
                let request = BFTProto::clone_from_slice(data.as_slice())?;
                let ret = self.verify_proto_obj(&request, true).await?;
                if !ret {
                    let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                        code: ERROR_SIGNATURE_ERROR as u32
                    }), Vec::new()).await?;
                    return resp.to_vec()
                }
                match request.desc().content() {
                    BFTProtoDescContent::PrepareRequest(req) => {
                        let body_ret = request.body();
                        if body_ret.is_none() {
                            log::error!("recv PrepareRequest none body");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_BLOCK_DECODE_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }

                        let block_ret = Block::clone_from_slice(body_ret.as_ref().unwrap().content().data.as_slice());
                        if block_ret.is_err() {
                            log::error!("recv PrepareRequest parse block failed");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_BLOCK_DECODE_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }

                        let block = block_ret.unwrap();
                        if req.block_id != block.desc().calculate_id() {
                            log::error!("recv PrepareRequest header block id not equal body block id");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_BLOCK_VERIFY_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }

                        let sender = self.sender.lock().unwrap();
                        sender.send(BFTMinerMsg::PrepareRequest(req.clone(), block)).unwrap();
                        Ok(Vec::new())
                    },
                    BFTProtoDescContent::PrepareResponse(req) => {
                        let sender = self.sender.lock().unwrap();
                        sender.send(BFTMinerMsg::PrepareResponse(req.clone())).unwrap();
                        Ok(Vec::new())
                    },
                    BFTProtoDescContent::ChangeView(req) => {
                        let sender = self.sender.lock().unwrap();
                        sender.send(BFTMinerMsg::ChangeView(req.clone())).unwrap();
                        Ok(Vec::new())
                    }
                    BFTProtoDescContent::Tx(tx_id) => {
                        let body_ret = request.body();
                        if body_ret.is_none() {
                            log::error!("recv Tx none body");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_BLOCK_DECODE_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }
                        let tx_ret = MetaTx::clone_from_slice(body_ret.as_ref().unwrap().content().data.as_slice());
                        if tx_ret.is_err() {
                            log::error!("recv Tx parse block failed");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_TX_DECODE_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }
                        let tx = tx_ret.unwrap();
                        if tx_id != &tx.desc().calculate_id() {
                            log::error!("recv tx id not equal body tx id");
                            let resp = self.new_proto_obj(BFTProtoDescContent::Error(BFTError {
                                code: ERROR_BLOCK_VERIFY_FAILED as u32
                            }), Vec::new()).await?;
                            return resp.to_vec()
                        }
                        self.on_recv_tx(&tx).await
                    },
                    BFTProtoDescContent::GetHeight => {
                        self.on_recv_get_height().await
                    },
                    BFTProtoDescContent::GetBlock(number) => {
                        self.on_recv_get_block(*number).await
                    },
                    BFTProtoDescContent::NodeSync(req) => {
                        self.on_recv_node_sync(req).await
                    }
                    _ => {
                        Ok(Vec::new())
                    }
                }
            } else {
                Err(meta_err!(ERROR_INVALID))
            }
        } else {
            Err(meta_err!(ERROR_INVALID))
        }
    }

    async fn sync_chain(&self, from: Option<String>) -> BuckyResult<()> {
        if from.is_none() && !self.network.has_connected().await? {
            log::info!("cur {} network is not connected.", self.self_index().await?);
            return Ok(());
        }
        loop {
            let height = {
                let ret = self.get_chain_height(from.clone()).await;
                if ret.is_ok() {
                    ret.unwrap()
                } else {
                    log::error!("get chain height err:{:?}", ret.err());
                    return Ok(());
                }
            };
            let ret = self.base.as_chain().get_chain_storage().get_tip_info().await;
            let (number, storage, archive_storage) = if let Err(e) = &ret {
                let code = get_meta_err_code(e)?;
                if code == ERROR_NOT_FOUND {
                    (-1 as i64, self.base.as_chain().get_chain_storage().state_storage(), self.base.as_chain().get_chain_storage().archive_storage())
                } else {
                    log::error!("get tip info err. code = {}", code);
                    return Err(meta_err!(code));
                }
            } else {
                let (block_desc, storage, archive_storage) = ret.as_ref().unwrap();
                (block_desc.number(), storage, archive_storage)
            };

            if height <= number {
                log::info!("cur {} height to {} dest height {}", self.self_index().await?, number, height);
                break;
            } else {
                let mut i = number + 1;
                while i >= 0 && i < (height + 1) {
                    let block = self.get_block(i, from.clone()).await?;
                    if self.verify_block_sign(&block).await? {
                        if i > 0 {
                            let _ = storage.recovery(i - 1).await;
                            log::info!("thread {:?} cur {} verify block {} {}",
                                       std::thread::current().id(),
                                       self.self_index().await?,
                                       block.desc().number(),
                                       block.desc().calculate_id().to_string());

                        }

                        if i > 1 {
                            let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
                            if &tip_desc.hash() != block.desc().pre_block_hash() {
                                self.base.as_chain().recovery(i - 2).await?;
                                i = i - 1;
                                continue;
                            }
                        }
                        let ret = BlockExecutor::execute_and_verify_block(&block,
                                                                          &storage,
                                                                          &archive_storage,
                                                                          Some(self.base.as_chain().get_chain_storage()),
                                                                          self.base.bfc_spv_node(),
                                                                          Some(self.miner_key.clone()),
                                                                          self.base.coinbase().clone()).await?;
                        if !ret {
                            // state_ref.rollback().await?;
                            log::error!("block {} verify failed", i);
                            return Err(meta_err!(ERROR_BLOCK_VERIFY_FAILED));
                        }
                        // state_ref.commit().await?;
                        if i > 0 {
                            for tx in block.transactions() {
                                self.base.remove(tx).await?;
                            }
                        }

                        self.base.as_chain().add_mined_block(&block).await?;
                        let _ = self.base.as_chain().backup(i).await;
                        if i > 0 {
                            log::info!("cur {} mined block {}",
                                       self.self_index().await?, block.desc().number());
                        }
                    } else {
                        log::error!("block {} signature verify failed", i);
                        return Err(meta_err!(ERROR_SIGNATURE_ERROR));
                    }
                    i += 1;
                }
            }
        }
        Ok(())
    }

    async fn start_mine_block(&self) -> BuckyResult<()> {
        assert_eq!(std::thread::current().id(), self.mining_thread.lock().unwrap().as_ref().unwrap().thread().id());
        let (block, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
        let view = {
            let status_info = self.status_info.lock().unwrap();
            if status_info.status != BFTMinerStatus::Init && status_info.status != BFTMinerStatus::ChangeViewSuccess {
                return Ok(())
            }
            if status_info.status == BFTMinerStatus::ChangeViewSuccess {
                if status_info.height > block.number() + 1 {
                    self.sync_chain(None).await?;
                }
            }
            status_info.view
        };
        let (speaker, device_desc) = self.get_next_block_miner(&block, view as u32).await?;
        if self.base.coinbase() == &(device_desc.calculate_id()) {
            {
                let mut status_info = self.status_info.lock().unwrap();
                log::info!("cur {} change status from {} to WaitingCreate", self.self_index().await?, status_info.status.to_string());
                status_info.change_status(BFTMinerStatus::WaitingCreate);
                status_info.speaker = Some(speaker as u8);
                status_info.speaker_desc = Some(device_desc);
                status_info.height = block.number() + 1;
            }
            self.reset_timer(TimerAction::CreateBlock, self.base.interval());
        } else {
            let view = {
                let mut status_info = self.status_info.lock().unwrap();
                log::info!("cur {} change status from {} to WaitingProposal", self.self_index().await?, status_info.status.to_string());
                status_info.change_status(BFTMinerStatus::WaitingProposal);
                status_info.speaker = Some(speaker as u8);
                status_info.speaker_desc = Some(device_desc);
                status_info.height = block.number() + 1;
                status_info.view
            };
            self.reset_timer(TimerAction::ChangeView(view), 2_u32.pow(view as u32 + 1) * self.base.interval());
        }
        Ok(())
    }

    async fn is_network_valid(&self) -> BuckyResult<bool> {
        let len = {
            let miners = self.get_miners().await?;
            miners.len()
        };

        Ok(self.network.get_node_list()?.len() >= ((0.7 * len as f32).ceil() as usize - 1))
    }

    async fn on_timeout(&self, action: TimerAction) -> BuckyResult<()> {
        assert_eq!(std::thread::current().id(), self.mining_thread.lock().unwrap().as_ref().unwrap().thread().id());
        match action {
            TimerAction::CreateBlock => {
                {
                    let status_info = self.status_info.lock().unwrap();
                    if status_info.status != BFTMinerStatus::WaitingCreate {
                        return Ok(())
                    }
                }
                let mut transactions = {
                    let pending = self.base.get_tx_pending_list().await;
                    pending.get_all()?
                };

                if self.chain_type == "bft" {
                    let ref_state = self.base.as_chain().get_chain_storage().state_storage().create_state(false).await;
                    let config = Config::new(&ref_state)?;
                    let btc_mint = BTCMint::new(&ref_state, &config, self.base.bfc_spv_node());
                    if let Ok(coinage_tx) = btc_mint.create_btc_coinage_record_tx() {
                        let nonce = self.base.get_nonce(self.base.coinbase()).await?;
                        let mut tx = MetaTx::new(nonce + 1, TxCaller::Id(self.base.coinbase().clone())
                                             , 0, 0, 0
                                             , None, MetaTxBody::BTCCoinageRecord(coinage_tx)
                                             , Vec::new()).build();
                        tx.async_sign(self.miner_key.clone()).await?;
                        transactions.push(tx);
                    }
                } else if self.chain_type == "bft_sub" {
                    let ref_state = self.base.as_chain().get_chain_storage().state_storage().create_state(false).await;
                    let config = Config::new(&ref_state)?;
                    let org_id = ref_state.config_get("miners_group", "").await?;
                    if org_id != "" {
                        let sub_chain_mint = SubChainMint::new(ObjectId::from_str(org_id.as_str())?,
                                                               &ref_state,
                                                               &config,
                                                               self.base.bfc_spv_node().to_owned());
                        if let Ok(coinage_tx) = sub_chain_mint.create_coinage_record_tx().await {
                            let nonce = self.base.get_nonce(self.base.coinbase()).await?;
                            let mut tx = MetaTx::new(nonce + 1, TxCaller::Id(self.base.coinbase().clone())
                                                 , 0, 0, 0
                                                 , None, MetaTxBody::SubChainCoinageRecord(coinage_tx)
                                                 , Vec::new()).build();
                            tx.async_sign(self.miner_key.clone()).await?;
                            transactions.push(tx);
                        }
                    }
                }

                let state_storage = self.base.as_chain().get_chain_storage().state_storage();
                let (tip_desc, _, _) = self.base.as_chain().get_chain_storage().get_tip_info().await?;
                let _ = state_storage.recovery(tip_desc.number()).await;

                log::info!("cur {} start mine block", self.self_index().await?);
                let mut block = self.base.create_block(transactions).await?;
                block.sign(self.miner_key.clone(), &SignatureSource::RefIndex(self.self_index().await?)).await?;

                let (view, speaker) = {
                    let status_info = self.status_info.lock().unwrap();
                    assert_eq!(status_info.status, BFTMinerStatus::WaitingCreate);
                    (status_info.view, status_info.speaker.unwrap())
                };
                let req = BFTPrepareRequest {
                    view,
                    speaker,
                    block_id: block.desc().calculate_id()
                };

                let proto = self.new_proto_obj(BFTProtoDescContent::PrepareRequest(req), block.to_vec()?).await?;
                log::info!("broadcast PrepareRequest begin");
                self.network.broadcast(proto.to_vec()?).await?;
                log::info!("broadcast PrepareRequest end");
                {
                    let mut status_info = self.status_info.lock().unwrap();
                    log::info!("cur {} change status from {} to WaitingAgree", self.self_index().await?, status_info.status.to_string());
                    status_info.change_status(BFTMinerStatus::WaitingAgree);
                    status_info.block = Some(block);
                }
            }
            TimerAction::ChangeView(last_view) => {
                let req = {
                    let mut status_info = self.status_info.lock().unwrap();
                    if status_info.status == BFTMinerStatus::Init || (status_info.status == BFTMinerStatus::ChangeViewSuccess
                    && bucky_time_now() - status_info.status_change_time < self.base.interval() as u64 * 1000000 ){
                        return Ok(());
                    }
                    assert_ne!(status_info.status, BFTMinerStatus::WaitingCreate);

                    let req = BFTChangeView {
                        height: status_info.height,
                        view: last_view,
                        member: self.self_index().await?,
                        dest_view: last_view + 1
                    };
                    status_info.view = last_view;

                    log::info!("cur {} ChangeView height {} view {} dest view {}", self.self_index().await?, status_info.height, status_info.view, req.dest_view);
                    req
                };
                let dest_view = req.dest_view;

                let proto = self.new_proto_obj(BFTProtoDescContent::ChangeView(req), Vec::new()).await?;
                log::info!("broadcast ChangeView begin");
                self.network.broadcast(proto.to_vec()?).await?;
                log::info!("broadcast ChangeView end");
                {
                    let mut status_info = self.status_info.lock().unwrap();
                    log::info!("cur {} status from {} to ChangeViewSent", self.self_index().await?, status_info.status.to_string());
                    status_info.change_status(BFTMinerStatus::ChangeViewSent);
                }
                self.reset_timer(TimerAction::ChangeView(dest_view), 2_u32.pow(dest_view as u32 + 1)*self.base.interval());
                self.check_change_view_reqs().await?;
            }
        }
        Ok(())
    }

    fn on_miner_msg(&self, msg: BFTMinerMsg) {
        async_std::task::block_on(async {
            let ret = match msg {
                BFTMinerMsg::StartMineBlock => {
                    self.start_mine_block().await
                }
                BFTMinerMsg::Timeout(action) => {
                    self.on_timeout(action).await
                }
                BFTMinerMsg::PrepareRequest(req, block) => {
                    self.on_recv_prepare_request(req, block).await
                }
                BFTMinerMsg::PrepareResponse(req) => {
                    self.on_recv_prepare_response(req).await
                }
                BFTMinerMsg::ChangeView(req) => {
                    self.on_recv_change_view(req).await
                }
                _ => {
                    log::error!("unreached here");
                    Ok(())
                }
            };
            if ret.is_err() {
                log::error!("on miner msg err: {:?}", ret.err().unwrap());
            }
        });
    }

    fn reset_timer(&self, action: TimerAction, interval: u32) {
        assert_eq!(std::thread::current().id(), self.mining_thread.lock().unwrap().as_ref().unwrap().thread().id());
        let sender = self.sender.lock().unwrap().clone();
        let mut timer = self.timer.lock().unwrap();
        *timer = Some(self.time_executor.lock().unwrap().schedule_with_delay(chrono::Duration::seconds(interval as i64), move || {
            sender.send(BFTMinerMsg::Timeout(action)).unwrap()
        }))
    }

    fn stop_timer(&self) {
        assert_eq!(std::thread::current().id(), self.mining_thread.lock().unwrap().as_ref().unwrap().thread().id());
        let mut guard = self.timer.lock().unwrap();
        guard.take();
    }

    fn get_mine_status_info(&self) -> MutexGuard<'_, BFTMinerStatusInfo>{
        self.status_info.lock().unwrap()
    }

    async fn node_sync(&self, from: Option<String>) -> BuckyResult<()> {
        let local_addr = self.network.local_addr().await?;
        let req_obj = self.new_proto_obj(BFTProtoDescContent::NodeSync(BFTNodeSync {
            node_id: self.base.coinbase().to_string(),
            addr: local_addr.clone()
        }), Vec::new()).await?;

        let data = req_obj.to_vec()?;
        let mut node_list = self.network.get_node_list()?;
        if from.is_some() {
            node_list.push(("".to_owned(), from.unwrap()))
        }
        while !node_list.is_empty() {
            let mut new_node_list = Vec::new();
            for (_, node) in &node_list {
                let ret_data = self.network.request(data.clone(), Some(node.clone())).await;
                if ret_data.is_err() {
                    log::info!("err {}", ret_data.err().unwrap());
                    continue
                }
                let ret_data = ret_data.unwrap();
                let obj = BFTProto::clone_from_slice(ret_data.as_slice())?;
                if let BFTProtoDescContent::NodeSyncResponse(resp) = obj.desc().content() {
                    self.network.add_node(resp.node_id.as_str(), node).await?;

                    for (new_node_id, new_node) in &resp.addr_list {
                        if self.base.coinbase().to_string() != new_node_id.to_owned() && !self.network.is_node_exist(new_node.as_str())? {
                            new_node_list.push((new_node_id.clone(), new_node.clone()));
                            self.network.add_node(new_node_id, new_node).await?;
                        }
                    }

                }
            }
            node_list = new_node_list;
        }

        Ok(())
    }
}

#[async_trait]
impl<NETWORK: ChainNetwork> Miner for BFTMiner<NETWORK> {
    fn as_chain(&self) -> &Chain {
        self.base.as_chain()
    }

    async fn push_tx(&self, tx: MetaTx) -> BuckyResult<()> {
        let proto = self.new_proto_obj(BFTProtoDescContent::Tx(tx.desc().calculate_id()), tx.to_vec()?).await?;
        self.base.push_tx(tx).await?;
        log::info!("broadcast tx begin");
        let ret = self.network.broadcast(proto.to_vec()?).await;
        log::info!("broadcast tx end");
        ret
    }

    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        self.base.get_nonce(account).await
    }

    fn get_interval(&self) -> u64 {
        self.base.interval() as u64
    }
}

impl<NETWORK: ChainNetwork> MinerRuner for BFTMiner<NETWORK> {
    fn run(self: &Arc<Self>) -> BuckyResult<()> {
        let miner = self.clone();

        async_std::task::block_on(async move {
            miner.sync_chain(None).await?;
            let weak_miner = Arc::downgrade(&miner);
            miner.network.start(move |data: Vec<u8>| {
                let weak_miner = weak_miner.clone();
                async move {
                    weak_miner.to_rc()?.on_recv(data).await
                }
            }).await?;

            self.node_sync(None).await
        })?;

        let miner = Arc::downgrade(self);
        let ret = std::thread::spawn(move || {
            let receiver = {
                let miner = miner.to_rc().unwrap();
                let mut guard = miner.receiver.lock().unwrap();
                guard.take()
            };
            for msg in receiver.unwrap().iter() {
                match msg {
                    BFTMinerMsg::Exit => {
                        log::info!("mine proc exit");
                        break;
                    }
                    _ => {
                        miner.to_rc().unwrap().on_miner_msg(msg);
                    }
                }
            }
        });
        let mut thread = self.mining_thread.lock().unwrap();
        *thread = Some(ret);

        let self_miner = self.clone();
        let ret: BuckyResult<()> = async_std::task::block_on(async move {
            let valid = self_miner.is_network_valid().await?;
            if valid {
                let status = {
                    let status_info = self_miner.status_info.lock().unwrap();
                    status_info.status
                };
                log::info!("cur {} status from {} to Init", self_miner.self_index().await?, status.to_string());
                let mut status_info = self_miner.status_info.lock().unwrap();
                status_info.change_status(BFTMinerStatus::Init);
                self_miner.sender.lock().unwrap().send(BFTMinerMsg::StartMineBlock).unwrap();
            } else {
                log::error!("check network error");
            }
            Ok(())
        });
        ret?;
        Ok(())
    }
}

impl<NETWORK: ChainNetwork> Drop for BFTMiner<NETWORK> {
    fn drop(&mut self) {
        self.sender.lock().unwrap().send(BFTMinerMsg::Exit).unwrap();
        let mut thread = self.mining_thread.lock().unwrap();
        if thread.is_some() {
            thread.take().unwrap().join().unwrap();
        }
        log::info!("BFTMiner drop {}", self.miner_key.public().to_hex().unwrap())
    }
}
//
// impl<NETWORK: ChainNetwork> Deref for BFTMiner<NETWORK> {
//     type Target = BaseMiner;
//
//     fn deref(&self) -> &Self::Target {
//         &self.base
//     }
// }
//
// impl<NETWORK: ChainNetwork> DerefMut for BFTMiner<NETWORK> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.base
//     }
// }

#[cfg(test)]
pub mod bft_miner_test {
    use crate::network::{ChainNetwork, ChainNetworkEventEndpoint, HttpTcpChainNetwork};
    use cyfs_base::*;
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use crate::chain::{BFTMiner, BlockExecutor, MinerRuner, BFTMinerStatus};
    use crate::{Chain, new_sql_storage, Miner};
    use std::fs::{remove_dir_all, create_dir_all};
    use std::path::{PathBuf, Path};
    use cyfs_base_meta::*;
    use crate::executor::context::{Config};
    use std::time::Duration;
    use std::convert::TryFrom;
    use crate::*;

    static mut INIT_LOG: bool = false;
    pub fn init_test_log() {
        unsafe {
            if !INIT_LOG {
                cyfs_base::init_log("test_bft_miner", None);
            }
            INIT_LOG = true;
        }
        // env_logger::builder().is_test(true).filter_level(LevelFilter::Debug).try_init().unwrap();
    }

    pub type MockChainNetworkManagerRef = Arc<MockChainNetworkManager>;

    pub struct MockChainNetworkManager {
        node_list: Mutex<Vec<(String, Arc<Box<dyn ChainNetworkEventEndpoint>>)>>,
    }

    unsafe impl Send for MockChainNetworkManager{}
    unsafe impl Sync for MockChainNetworkManager{}

    impl MockChainNetworkManager {
        pub fn new() -> MockChainNetworkManagerRef {
            Arc::new(MockChainNetworkManager {
                node_list: Mutex::new(Vec::new()),
            })
        }

        pub async fn add_ep(&self, id: String, ep: impl ChainNetworkEventEndpoint) -> BuckyResult<()> {
            let mut node_list = self.node_list.lock().unwrap();
            node_list.push((id, Arc::new(Box::new(ep))));
            Ok(())
        }

        pub async fn send(&self, _from: &str, to: &str, data: Vec<u8>) -> BuckyResult<()> {
            let node_list = self.node_list.lock().unwrap();
            for (_id, node) in node_list.iter() {
                let node_tmp = node.clone();
                if to == _id {
                    let send_data = data.clone();
                    async_std::task::spawn(async move {
                        node_tmp.call(send_data).await.unwrap();
                    });
                }
            }
            Ok(())
        }

        async fn request(&self, id: &str, data: Vec<u8>) -> BuckyResult<Vec<u8>> {
            let node_list = {
                let list = self.node_list.lock().unwrap();
                list.clone()
            };
            for (_id, node) in node_list.iter() {
                if id == _id {
                    return node.call(data.clone()).await;
                }
            }
            Err(meta_err!(ERROR_NETWORK_ERROR))
        }
    }

    pub struct MockChainNetwork {
        pub id: String,
        pub node_list: Mutex<Vec<(String, String)>>,
        network_manager: MockChainNetworkManagerRef,
    }
    unsafe impl Send for MockChainNetwork {}
    unsafe impl Sync for MockChainNetwork {}

    static mut INDEX: i32 = 0;

    impl MockChainNetwork {
        pub fn new(manager: &MockChainNetworkManagerRef) -> MockChainNetwork {
            unsafe {
                INDEX = INDEX + 1;

                MockChainNetwork {
                    id: format!("{}", INDEX),
                    node_list: Mutex::new(Vec::new()),
                    network_manager: manager.clone(),
                }
            }
        }

        pub fn get_node_id(&self) -> &str {
            self.id.as_str()
        }
    }

    #[async_trait]
    impl ChainNetwork for MockChainNetwork {
        async fn broadcast(&self, obj: Vec<u8>) -> BuckyResult<()> {
            let node_list = {
                let list = self.node_list.lock().unwrap();
                list.clone()
            };
            for (_, node) in &node_list {
                self.network_manager.send(self.id.as_str(), node.as_str(), obj.clone()).await?;
            }
            Ok(())
        }

        async fn request(&self, param: Vec<u8>, to: Option<String>) -> BuckyResult<Vec<u8>> {
            if to.is_some() {
                self.network_manager.request(to.unwrap().as_str(), param.clone()).await
            } else {
                let node_list = {
                    let list = self.node_list.lock().unwrap();
                    list.clone()
                };
                if node_list.len() == 0 {
                    return Err(meta_err!(ERROR_NETWORK_ERROR));
                }

                for (_, node) in &node_list {
                    let ret = self.network_manager.request(node, param.clone()).await;
                    if ret.is_ok() {
                        return ret;
                    }
                }
                Err(meta_err!(ERROR_NETWORK_ERROR))
            }
            }

        async fn start(&self, ep: impl ChainNetworkEventEndpoint) -> BuckyResult<()> {
            self.network_manager.add_ep(self.id.clone(), ep).await
        }

        async fn stop(&self) -> BuckyResult<()> {
            Ok(())
        }

        async fn has_connected(&self) -> BuckyResult<bool> {
            Ok(self.node_list.lock().unwrap().len() != 0)
        }

        async fn local_addr(&self) -> BuckyResult<String> {
            Ok(self.get_node_id().to_owned())
        }

        async fn is_local_addr(&self, node: &str) -> BuckyResult<bool> {
            Ok(self.get_node_id() == node)
        }

        fn get_node_list(&self) -> BuckyResult<Vec<(String, String)>> {
            Ok(self.node_list.lock().unwrap().clone())
        }

        fn is_node_exist(&self, node: &str) -> BuckyResult<bool> {
            let node_list = self.node_list.lock().unwrap();
            for (_, inner_node) in node_list.iter() {
                if node == inner_node {
                    return Ok(true);
                }
            }
            Ok(false)
        }

        async fn add_node(&self, node_id: &str, node: &str) -> BuckyResult<()> {
            let mut node_list = self.node_list.lock().unwrap();
            node_list.push((node_id.to_owned(), node.to_owned()));
            Ok(())
        }

        fn get_node(&self, node_id: &str) -> Option<String> {
            let node_list = self.node_list.lock().unwrap();
            for (id, node) in node_list.iter() {
                if id == node_id {
                    return Some(node.to_owned());
                }
            }
            None
        }
    }

    pub fn create_miner_device_info_list(count: i32) -> Vec<(Device, PrivateKey)> {
        let mut list = Vec::new();
        for _ in 0..count {
            let private_key = PrivateKey::generate_rsa(1024).unwrap();
            let device = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key.public()
                , Area::default()
                , DeviceCategory::Server).build();
            list.push((device, private_key));
        }
        list
    }

    pub fn create_bft_org(device_list: &Vec<(Device, PrivateKey)>) -> BuckyResult<MinerGroup> {
        let mut members = Vec::new();
        for (device, _) in device_list {
            members.push(device.desc().clone());
        }

        let mut miner_group = MinerGroup::new(members).build();
        for (device, private_key) in device_list {
            let signer = RsaCPUObjectSigner::new(private_key.public(), private_key.clone());
            async_std::task::block_on(sign_and_push_named_object(&signer,
                                                                &mut miner_group,
                                                                &SignatureSource::Object(ObjectLink { obj_id: device.desc().calculate_id(), obj_owner: None })))?;
        }

        Ok(miner_group)
    }

    async fn create_test_miner(test_id: &str,
                               miner_group: MinerGroup,
                               device_list: &Vec<(Device, PrivateKey)>,
                               index: usize,
                               interval: u32,
                               manager: &MockChainNetworkManagerRef) -> BuckyResult<(Arc<BFTMiner<MockChainNetwork>>, String)> {
        let (device, private_key) = device_list.get(index).unwrap();
        let device_id = device.desc().calculate_id();
        let network = MockChainNetwork::new(manager);

        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("bftminer/miner_{}_{}", test_id, index));
        println!("{}", temp_dir.to_str().unwrap());
        if temp_dir.exists() {
            remove_dir_all(temp_dir.clone()).unwrap();
        }
        create_dir_all(temp_dir.clone()).unwrap();

        let storage = new_sql_storage(temp_dir.join("state_db").as_path());
        let header = BlockDesc::new(BlockDescContent::new(device_id, None)).build();
        let mut block_body = BlockBody::new();
        let state = storage.create_state(false).await;
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await?;
        let meta_config = Config::new(&state)?;
        state.create_cycle_event_table(meta_config.get_rent_cycle()).await?;

        let caller = TxCaller::Device(device.desc().clone());
        // for (device, _) in device_list {
        //     let create_desc = CreateDescTx {
        //         coin_id: 0,
        //         from: None,
        //         value: 0,
        //         desc: SavedMetaObject::Device(device.clone()),
        //         price: 0
        //     };
        //     let mut tx = Tx::new(nonce,
        //             caller.clone(),
        //             0,
        //             0,
        //             0,
        //             None,
        //             TxBody::CreateDesc(create_desc),
        //             Vec::new()).build();
        //     tx.sign(private_key)?;
        //
        //     nonce += 1;
        //
        //     block_body.add_transaction(tx).unwrap();
        // }

        let meta_create_tx = MetaTxBody::CreateMinerGroup(miner_group);
        let mut tx = MetaTx::new(1,
        caller.clone(),
        0,
        0,
        0,
        None,
                                 meta_create_tx,
        Vec::new()).build();
        tx.sign(private_key.clone())?;
        block_body.add_transaction(tx).unwrap();

        // state.being_transaction().await?;
        let ret = BlockExecutor::execute_block(&header, &mut block_body, &state, &meta_config, None, "".to_string(), None, ObjectId::default()).await;
        if ret.is_ok() {
            // state.commit().await?;
        } else {
            // state.rollback().await?;
            ret?;
        }

        let state_hash = storage.state_hash().await?;

        let mut block = Block::new(device_id.clone(), None, state_hash, block_body)?.build();
        block.sign(private_key.clone(), &SignatureSource::Key(PublicKeyValue::Single(private_key.public()))).await?;

        let chain = Chain::new(PathBuf::from(temp_dir),
                               Some(block),
                               storage).await?;

        let addr = network.local_addr().await?;
        let miner = Arc::new(BFTMiner::new("bft".to_owned(),
                                           device_id,
                                           interval,
                                           chain,
                                           "".to_string(),
                                           network,
                                           private_key.clone())?);
        miner.run()?;
        Ok((miner, addr))
    }

    async fn create_other_miner(test_id: &str,
                                other_miner: String,
                                device_list: &Vec<(Device, PrivateKey)>,
                                index: usize,
                                interval: u32,
                                manager: &MockChainNetworkManagerRef) -> BuckyResult<(Arc<BFTMiner<MockChainNetwork>>, String)> {
        let (device, private_key) = device_list.get(index).unwrap();
        let device_id = device.desc().calculate_id();
        let network = MockChainNetwork::new(manager);
        network.add_node("unknown",other_miner.as_str()).await?;

        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("bftminer/miner_{}_{}", test_id, index));
        println!("{}", temp_dir.to_str().unwrap());
        if temp_dir.exists() {
            remove_dir_all(temp_dir.clone()).unwrap();
        }
        create_dir_all(temp_dir.clone()).unwrap();

        let storage = new_sql_storage(temp_dir.join("state_db").as_path());
        let state = storage.create_state(false).await;
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await?;
        let meta_config = Config::new(&state)?;
        state.create_cycle_event_table(meta_config.get_rent_cycle()).await?;

        let chain = Chain::new(PathBuf::from(temp_dir),
                               None,
                               storage).await?;

        let addr = network.local_addr().await?;
        let miner = Arc::new(BFTMiner::new("bft".to_owned(),
                                           device_id,
                                           interval,
                                           chain,
                                           "".to_string(),
                                           network,
                                           private_key.clone())?);
        miner.run()?;
        Ok((miner, addr))
    }

    async fn create_test_http_miner(test_id: &str,
                                    miner_group: MinerGroup,
                                    device_list: &Vec<(Device, PrivateKey)>,
                                    index: usize,
                                    interval: u32,
                                    port: u16) -> BuckyResult<(Arc<BFTMiner<HttpTcpChainNetwork>>, String)> {
        let (device, private_key) = device_list.get(index).unwrap();
        let device_id = device.desc().calculate_id();
        let network = HttpTcpChainNetwork::new(port, Vec::new());

        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("bftminer/miner_{}_{}", test_id, index));
        println!("{}", temp_dir.to_str().unwrap());
        if temp_dir.exists() {
            remove_dir_all(temp_dir.clone()).unwrap();
        }
        create_dir_all(temp_dir.clone()).unwrap();

        let storage = new_sql_storage(temp_dir.join("state_db").as_path());
        let header = BlockDesc::new(BlockDescContent::new(device_id, None)).build();
        let mut block_body = BlockBody::new();
        let state = storage.create_state(false).await;
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await?;
        let meta_config = Config::new(&state)?;
        state.create_cycle_event_table(meta_config.get_rent_cycle()).await?;

        let caller = TxCaller::Device(device.desc().clone());
        let id = caller.id();
        let mut nonce = 1;
        for (device, _) in device_list {
            let saved_obj = SavedMetaObject::Device(device.clone());
            let create_desc = CreateDescTx {
                coin_id: 0,
                from: None,
                value: 0,
                desc_hash: saved_obj.hash()?,
                price: 0
            };
            let mut tx = MetaTx::new(nonce,
                                 caller.clone(),
                                 0,
                                 0,
                                 0,
                                 None,
                                     MetaTxBody::CreateDesc(create_desc),
                                 saved_obj.to_vec()?).build();
            tx.sign(private_key.clone())?;

            nonce += 1;

            block_body.add_transaction(tx).unwrap();
        }

        let meta_create_tx = MetaTxBody::CreateMinerGroup(miner_group);
        let mut tx = MetaTx::new(nonce,
                             caller.clone(),
                             0,
                             0,
                             0,
                             None,
                                 meta_create_tx,
                             Vec::new()).build();
        tx.sign(private_key.clone())?;
        block_body.add_transaction(tx).unwrap();

        let id = caller.id();
        // state.being_transaction().await?;
        let ret = BlockExecutor::execute_block(&header, &mut block_body, &state, &meta_config, None, "".to_owned(), None, ObjectId::default()).await;
        if ret.is_ok() {
            // state.commit().await?;
        } else {
            // state.rollback().await?;
            ret?;
        }

        let state_hash = storage.state_hash().await?;

        let mut block = Block::new(device_id.clone(), None, state_hash, block_body)?.build();
        block.sign(private_key.clone(), &SignatureSource::Key(PublicKeyValue::Single(private_key.public()))).await?;

        let chain = Chain::new(PathBuf::from(temp_dir),
                               Some(block),
                               storage).await?;

        let addr = network.local_addr().await?;
        let miner = Arc::new(BFTMiner::new("bft".to_owned(),
                                           device_id,
                                           interval,
                                           chain,
                                           "".to_string(),
                                           network,
                                           private_key.clone())?);
        miner.run()?;
        Ok((miner, addr))
    }

    async fn create_other_http_miner(test_id: &str,
                                device_list: &Vec<(Device, PrivateKey)>,
                                other_miner: &str,
                                index: usize,
                                interval: u32,
                                port: u16) -> BuckyResult<(Arc<BFTMiner<HttpTcpChainNetwork>>, String)> {
        let (device, private_key) = device_list.get(index).unwrap();
        let device_id = device.desc().calculate_id();
        let network = HttpTcpChainNetwork::new(port, Vec::new());
        network.add_node("unknown", other_miner).await?;

        let mut temp_dir = std::env::temp_dir();
        temp_dir.push(format!("bftminer/miner_{}_{}", test_id, index));
        println!("{}", temp_dir.to_str().unwrap());
        if temp_dir.exists() {
            remove_dir_all(temp_dir.clone()).unwrap();
        }
        create_dir_all(temp_dir.clone()).unwrap();

        let storage = new_sql_storage(temp_dir.join("state_db").as_path());
        let state = storage.create_state(false).await;
        state.init_genesis(&vec![GenesisCoinConfig {
            coin_id: 0,
            pre_balance: vec![]
        }]).await?;
        let meta_config = Config::new(&state)?;
        state.create_cycle_event_table(meta_config.get_rent_cycle()).await?;

        let chain = Chain::new(PathBuf::from(temp_dir),
                               None,
                               storage).await?;

        let addr = network.local_addr().await?;
        let miner = Arc::new(BFTMiner::new("bft".to_owned(),
                                           device_id,
                                           interval,
                                           chain,
                                           "".to_string(),
                                           network,
                                           private_key.clone())?);
        miner.run()?;
        Ok((miner, addr))
    }

    async fn create_test_miner_list(test_id: &str, node_num: i32) -> Vec<Arc<BFTMiner<MockChainNetwork>>> {
        let manager = MockChainNetworkManager::new();
        let device_list = create_miner_device_info_list(node_num);
        let org = create_bft_org(&device_list).unwrap();
        let mut node_list = Vec::new();

        let (ret, addr) = create_test_miner(test_id, org, &device_list, 0, 10, &manager).await.unwrap();
        node_list.push(ret);
        for i in 1..node_num {
            let (miner, _) = create_other_miner(test_id, addr.clone(), &device_list, i as usize, 10, &manager).await.unwrap();
            node_list.push(miner);
        }
        node_list
    }

    pub async fn create_test_http_miner_list(test_id: &str, node_num: i32, port: u16, interval: u32) -> Vec<Arc<BFTMiner<HttpTcpChainNetwork>>> {
        let device_list = create_miner_device_info_list(node_num);
        let org = create_bft_org(&device_list).unwrap();
        let mut node_list = Vec::new();

        let (ret, addr) = create_test_http_miner(test_id, org, &device_list, 0, interval, port).await.unwrap();
        node_list.push(ret);
        async_std::task::sleep(Duration::new(2, 0)).await;
        for i in 1..node_num {
            let (miner, _) = create_other_http_miner(test_id, &device_list, addr.as_str(), i as usize, interval, port + i as u16).await.unwrap();
            node_list.push(miner);
        }
        node_list
    }

    #[test]
    fn test_bft_single_miner_run() {
        init_test_log();
        async_std::task::block_on(async {
            let manager = MockChainNetworkManager::new();
            let device_list = create_miner_device_info_list(7);
            let org = create_bft_org(&device_list).unwrap();
            let ret = create_test_miner("test_single_miner",org, &device_list, 0, 10, &manager).await;
            assert!(ret.is_ok());
            let (miner, _) = ret.unwrap();
            {
                let status_info = miner.get_mine_status_info();
                assert_eq!(status_info.status, BFTMinerStatus::None, "{}", status_info.status.to_string());
            }

            async_std::task::sleep(Duration::new(25, 0)).await;
            {
                let status_info = miner.get_mine_status_info();
                assert_eq!(status_info.status, BFTMinerStatus::None, "{}", status_info.status.to_string());
            }
        });
    }

    #[test]
    fn test_bft_miner() {
        async_std::task::block_on(async {
            let miner_list = create_test_miner_list("bft_miner", 7).await;
            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 0);

                let status_info = miner.get_mine_status_info();
                if i == 1 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(14, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 1);

                let status_info = miner.get_mine_status_info();
                if i == 2 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(11, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 2);

                let status_info = miner.get_mine_status_info();
                if i == 3 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(11, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 3);

                let status_info = miner.get_mine_status_info();
                if i == 4 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(11, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 4);

                let status_info = miner.get_mine_status_info();
                if i == 5 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(11, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 5);

                let status_info = miner.get_mine_status_info();
                if i == 6 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(11, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 6);

                let status_info = miner.get_mine_status_info();
                if i == 0 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }
        });
    }

    #[test]
    fn test_change_view() {
        async_std::task::block_on(async {
            let node_num = 7;
            let test_id = "change_view";
            let manager = MockChainNetworkManager::new();
            let device_list = create_miner_device_info_list(node_num);
            let org = create_bft_org(&device_list).unwrap();
            let mut miner_list = Vec::new();

            let (ret, addr) = create_test_miner(test_id, org, &device_list, 0, 15, &manager).await.unwrap();
            miner_list.push(ret);
            for i in 1..node_num {
                if i != 1 {
                    let (miner, _) = create_other_miner(test_id, addr.clone(), &device_list, i as usize, 15, &manager).await.unwrap();
                    miner_list.push(miner);
                }
            }

            async_std::task::sleep(Duration::new(25, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 0);

                let status_info = miner.get_mine_status_info();
                assert_eq!(status_info.status, BFTMinerStatus::WaitingProposal);
            }

            async_std::task::sleep(Duration::new(17, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 0);

                let status_info = miner.get_mine_status_info();
                if i == 1 {
                    assert_eq!(status_info.status, BFTMinerStatus::WaitingCreate);
                } else {
                    assert_eq!(status_info.status, BFTMinerStatus::WaitingProposal);
                }
            }

            async_std::task::sleep(Duration::new(15, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 1);

                let status_info = miner.get_mine_status_info();
                if i == 2 {
                    assert_eq!(status_info.status, BFTMinerStatus::WaitingCreate);
                } else {
                    assert_eq!(status_info.status, BFTMinerStatus::WaitingProposal);
                }
            }
        });
    }

    #[test]
    fn test_bft_http_miner() {
        init_test_log();
        async_std::task::block_on(async {
            let miner_list = create_test_http_miner_list("bft_http_miner", 7, 5679, 20).await;
            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 0);

                let status_info = miner.get_mine_status_info();
                if i == 1 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(30, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 1);

                let status_info = miner.get_mine_status_info();
                if i == 2 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 2);

                let status_info = miner.get_mine_status_info();
                if i == 3 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 3);

                let status_info = miner.get_mine_status_info();
                if i == 4 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 4);

                let status_info = miner.get_mine_status_info();
                if i == 5 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 5);

                let status_info = miner.get_mine_status_info();
                if i == 6 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 6);

                let status_info = miner.get_mine_status_info();
                if i == 0 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }
        });
    }

    #[test]
    fn test_bft_tx_transmission() {
        init_test_log();
        async_std::task::block_on(async {
            let miner_list = create_test_http_miner_list("bft_tx_transmission", 7, 5779, 20).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];

                let (block_desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                assert_eq!(block_desc.number(), 0);

                let status_info = miner.get_mine_status_info();
                if i == 1 {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingCreate, "{}", status_info.status.to_string());
                } else {
                    assert!(status_info.status == BFTMinerStatus::Init || status_info.status == BFTMinerStatus::WaitingProposal, "{}", status_info.status.to_string());
                }
            }

            async_std::task::sleep(Duration::new(24, 0)).await;

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key1.public()
                , Area::default()
                , DeviceCategory::OOD).build();

            let mut tx = MetaTx::new(
                1
                , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                , 0
                , 0
                , 0
                , None
                , MetaTxBody::BidName(BidNameTx {
                    name: "test".to_string(),
                    owner: None,
                    name_price: 1,
                    price: 1
                })
                , Vec::new()).build();
            let ret = tx.sign(private_key1);
            assert!(ret.is_ok());

            let ret = miner_list[0].push_tx(tx).await;
            assert!(ret.is_ok());

            async_std::task::sleep(Duration::new(3, 0)).await;

            for miner in &miner_list {
                let pending_list = miner.base.get_tx_pending_list().await;
                let tx_list = pending_list.get_all().unwrap();
                assert_eq!(tx_list.len(), 1);
            }
        });
    }

    #[test]
    fn test_bft_block_verify() {
        init_test_log();
        async_std::task::block_on(async {
            let miner_list = create_test_http_miner_list("bft_block_verify", 7, 6779, 1).await;

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key1.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let _id1 = device1.desc().calculate_id();

            let private_key2 = PrivateKey::generate_rsa(1024).unwrap();
            let _device2 = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key2.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let id2 = device1.desc().calculate_id();

            for nonce in 1..50 {
                let mut tx = MetaTx::new(
                    nonce
                    , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                    , 0
                    , 0
                    , 0
                    , None
                    , MetaTxBody::TransBalance(TransBalanceTx {
                        ctid: CoinTokenId::Coin(0),
                        to: vec![(id2, 100)]
                    })
                    , Vec::new()).build();
                let ret = tx.sign(private_key1.clone());
                assert!(ret.is_ok());

                let ret = miner_list[0].push_tx(tx).await;
                assert!(ret.is_ok());

                // let (any, _) = AnyNamedObject::decode_from_file(Path::new(r#"C:\Users\wugren\Desktop\cyfs_web.obj"#), &mut Vec::new()).unwrap();
                // let saved = SavedMetaObject::Data(Data {
                //     id: any.calculate_id(),
                //     data: any.to_vec().unwrap()
                // });
                // let mut tx = MetaTx::new(
                //     nonce
                //     , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                //     , 0
                //     , 0
                //     , 0
                //     , None
                //     , MetaTxBody::CreateDesc(CreateDescTx {
                //         coin_id: 0,
                //         from: None,
                //         value: 0,
                //         desc_hash: saved.hash().unwrap(),
                //         price: 0
                //     })
                //     , any.to_vec().unwrap()).build();
                // let ret = tx.sign(private_key1.clone());
                // assert!(ret.is_ok());
                //
                // let ret = miner_list[0].push_tx(tx).await;
                // assert!(ret.is_ok());

                async_std::task::sleep(Duration::new(1, 0)).await;
            }

            async_std::task::sleep(Duration::new(20, 0)).await;

            for i in 0..miner_list.len() {
                let miner = &miner_list[i];
                let (desc, _) = miner.as_chain().get_chain_storage().get_tip_info().await.unwrap();
                println!("{} height {}", i, desc.number());
            }
        });
    }

}
