use std::{
    sync::{Arc, Mutex}, 
    time::Duration, 
    str::FromStr, 
    collections::{LinkedList, HashMap}
};
use async_std::{
    task, 
    future
};
use async_recursion::async_recursion;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_lib::*;
use cyfs_util::*;
use cyfs_dsg_client::*;


pub struct DsgServiceConfig {
    pub initial_challenge: DsgChallengeOptions, 
    pub store_challenge: DsgChallengeOptions, 
    pub challenge_interval: Duration,  
    pub repost_challenge_interval: Duration, 
    pub atomic_interval: Duration, 
    pub merge_chunk_size: u64, 
    pub split_chunk_size: usize
}

impl Default for DsgServiceConfig {
    fn default() -> Self {
        Self {
            atomic_interval: Duration::from_secs(60), 
            initial_challenge: DsgChallengeOptions {
                sample_count: 2, 
                sample_len: 16 * 1024,
                live_time: Duration::from_secs(24 * 3600)
            }, 
            store_challenge: DsgChallengeOptions {
                sample_count: 1, 
                sample_len: 16 * 1024,
                live_time: Duration::from_secs(1 * 3600)
            }, 
            repost_challenge_interval: Duration::from_secs(60), 
            challenge_interval: Duration::from_secs(24 * 3600), 
            merge_chunk_size: u32::MAX as u64, 
            split_chunk_size: 18 * 1024 * 1024
        }
    }
}

struct ChallengeState {
    challenge: ObjectId, 
    post_at: u64, 
    posted_at: Option<u64>
}

struct ContractState {
    challenge: Option<ChallengeState>
}

struct ContractStates(Mutex<HashMap<ObjectId, ContractState>>);

impl ContractStates {
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn pre_post_challenge<'a>(&self, challenge: DsgChallengeObjectRef<'a>, config: &DsgServiceConfig) -> bool {
        let mut contracts = self.0.lock().unwrap();
        let contract_state = {
            if let Some(state) = contracts.get_mut(challenge.contract_id()) {
                state
            } else {
                contracts.insert(challenge.contract_id().clone(), ContractState { challenge: None });
                contracts.get_mut(challenge.contract_id()).unwrap()
            }
        };
        if let Some(challenge_state) = contract_state.challenge.as_mut() {
            if challenge_state.challenge != challenge.id() {
                false 
            } else {
                if challenge_state.posted_at.is_some() {
                    false
                } else {
                    let now = bucky_time_now();
                    if now > challenge_state.post_at 
                        && Duration::from_micros(now - challenge_state.post_at) > config.repost_challenge_interval {
                        challenge_state.post_at = now;
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            contract_state.challenge = Some(ChallengeState {
                challenge: challenge.id(), 
                post_at: bucky_time_now(), 
                posted_at: None
            });
            true
        }
    }

    fn post_post_challenge<'a>(&self, challenge: DsgChallengeObjectRef<'a>) {
        let mut contracts = self.0.lock().unwrap();
        if let Some(contract_state) = contracts.get_mut(challenge.contract_id()) {
            if let Some(challenge_state) = contract_state.challenge.as_mut() {
                if challenge_state.challenge == challenge.id() {
                    challenge_state.posted_at = Some(bucky_time_now())
                }
            }
        }
    }

    fn finish_challenge<'a>(&self, challenge: DsgChallengeObjectRef<'a>) {
        self.finish_challenge_id(challenge.contract_id(), challenge.id())
    }

    fn finish_challenge_id(&self, contract_id: &ObjectId, challenge_id: ObjectId) {
        let mut contracts = self.0.lock().unwrap();
        if let Some(contract_state) = contracts.get_mut(contract_id) {
            if let Some(challenge_state) = &mut contract_state.challenge {
                if challenge_state.challenge == challenge_id {
                    contract_state.challenge = None;
                }
            }
        }
    }

    fn finish_contract(&self, contract: ObjectId) {
        let mut contracts = self.0.lock().unwrap();
        contracts.remove(&contract);
    }
}

struct ServiceImpl {
    config: DsgServiceConfig, 
    stack: Arc<SharedCyfsStack>, 
    contracts: ContractStates
}

#[derive(Clone)]
pub struct DsgService(Arc<ServiceImpl>);

impl std::fmt::Display for DsgService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgService")
    }
}


impl DsgService {
    pub async fn new(stack: Arc<SharedCyfsStack>, config: DsgServiceConfig) -> BuckyResult<Self> {
        if config.split_chunk_size % DsgDataSourceStubObjectRef::enc_block_len() != 0 {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "config split not n * key length"));
        }
        let service = Self(Arc::new(ServiceImpl {
            config, 
            stack, 
            contracts: ContractStates::new()
        }));
        
        let _ = service.listen()?;
        let _ = service.stack().wait_online(None).await?;

        {
            let service = service.clone();
            task::spawn(async move {
                loop {
                    let now = bucky_time_now();
                    // log::debug!("{} on time escape, at={}", service, now);
                    let _ = service.on_time_escape(now).await;
                    let _ = future::timeout(service.config().atomic_interval, future::pending::<()>()).await;
                }
            });
        }
        
        Ok(service)
    }

    fn stack(&self) -> &SharedCyfsStack {
        &self.0.stack.as_ref()
    }

    fn chunk_reader(&self) -> Box<dyn ChunkReader> {
        DsgStackChunkReader::new(self.0.stack.clone()).clone_as_reader()
    }

    fn config(&self) -> &DsgServiceConfig {
        &self.0.config
    }

    fn contracts(&self) -> &ContractStates {
        &self.0.contracts
    }

    // 加载中间的数据状态
    fn load(&self) -> BuckyResult<()> {
        Ok(())
    }

    fn listen(&self) -> BuckyResult<()> {
        // post contract state
        struct OnSyncContractState {
            service: DsgService
        }

        #[async_trait]
        impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult> for OnSyncContractState {
            async fn call(&self, param: &RouterHandlerPostObjectRequest) -> BuckyResult<RouterHandlerPostObjectResult> {
                log::info!("{} OnSyncContractState called, id = {} from = {}", self.service, param.request.object.object_id, param.request.common.source);
                let state = DsgContractStateObject::clone_from_slice(param.request.object.object_raw.as_slice())
                    .map_err(|err| {
                        log::error!("{} OnSyncContractState failed, id={} from={} err=decode state object {}", self.service, param.request.object.object_id, param.request.common.source, err);
                        err 
                    })?;
                let new_state = self.service.sync_contract_state(state, Some(param.request.common.source.object_id().clone())).await?;
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response, 
                    request: None, 
                    response: Some(Ok(NONPostObjectInputResponse {
                        object: Some(NONObjectInfo {
                            object_id: DsgContractStateObjectRef::from(&new_state).id(), 
                            object_raw: new_state.to_vec()?, 
                            object: None
                        })
                    }))
                })
            }
        }

        let _ = self.stack().router_handlers().add_handler(
            RouterHandlerChain::Handler, 
            "OnSyncContractState", 
            0, 
            format!("obj_type == {} && object.dec_id == {}",  DsgContractStateDesc::obj_type(), dsg_dec_id()).as_str(), 
            RouterHandlerAction::Default, 
            Some(Box::new(OnSyncContractState {service: self.clone()})))
            .map_err(|err| {
                log::error!("{} listen failed, err=register OnSyncContractState handler {}", self, err);
                err
            })?;


        // post proof
        struct OnProof {
            service: DsgService
        }

        #[async_trait]
        impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult> for OnProof {
            async fn call(&self, param: &RouterHandlerPostObjectRequest) -> BuckyResult<RouterHandlerPostObjectResult> {
                log::info!("{} OnProof called, id = {} from = {}", self.service, param.request.object.object_id, param.request.common.source);
                let proof = DsgProofObject::clone_from_slice(param.request.object.object_raw.as_slice()).map_err(|err| {
                    log::error!("{} OnProof failed, id={} from={} err=decode proof object {}", self.service, param.request.object.object_id, param.request.common.source, err);
                    err 
                })?;
                let signed_proof = self.service.on_proof(DsgProofObjectRef::from(&proof)).await;
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response, 
                    request: None, 
                    response: Some(signed_proof.map(|proof| {
                        NONPostObjectInputResponse {
                            object: Some(NONObjectInfo {
                                object_id: DsgProofObjectRef::from(&proof).id(), 
                                object_raw: proof.to_vec().unwrap(), 
                                object: None
                            })
                        }
                    }))
                })
            }
        }

        let _ = self.stack().router_handlers().add_handler(
            RouterHandlerChain::Handler, 
            "OnProof", 
            0, 
            format!("obj_type == {} && object.dec_id == {}",  DsgProofDesc::obj_type(), dsg_dec_id()).as_str(), 
            RouterHandlerAction::Default, 
            Some(Box::new(OnProof {service: self.clone()})))
            .map_err(|err| {
                log::error!("{} listen failed, err=register OnProof handler {}", self, err);
                err
            })?;

        
        // post query
        struct OnQuery {
            service: DsgService
        }

        #[async_trait]
        impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult> for OnQuery {
            async fn call(&self, param: &RouterHandlerPostObjectRequest) -> BuckyResult<RouterHandlerPostObjectResult> {
                let query = DsgQueryObject::clone_from_slice(param.request.object.object_raw.as_slice())?;
                let resp = self.service.on_query(DsgQuery::try_from(query)?).await?;
                let resp_obj: DsgQueryObject = resp.into();
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response, 
                    request: None, 
                    response: Some(Ok(NONPostObjectInputResponse {
                        object: Some(NONObjectInfo {
                            object_id: resp_obj.desc().object_id(), 
                            object_raw: resp_obj.to_vec()?, 
                            object: None
                        })
                    }))
                })
            }
        }

        let _ = self.stack().router_handlers().add_handler(
            RouterHandlerChain::Handler,
            "OnQuery", 
            0, 
            format!("obj_type == {} && object.dec_id == {}",  DsgQueryDesc::obj_type(), dsg_dec_id()).as_str(), 
            RouterHandlerAction::Default, 
            Some(Box::new(OnQuery {service: self.clone()})))
            .map_err(|err| {
                log::error!("{} listen failed, err=register OnQuery handler {}", self, err);
                err
            })?;

        Ok(())
    } 

    pub(crate) async fn query_contracts(&self, skip: u32, limit: Option<u32>) -> BuckyResult<HashMap<ObjectId, ObjectId>> {
        let op = self.stack().root_state_stub(None, None).create_single_op_env().await?;
        op.load_by_path("/dsg-service/contracts/").await?;
        let _ = op.next(skip).await?;
        let states = if let Some(limit) = limit {
            let iter = op.next(limit).await?;
            HashMap::from_iter(
                iter.into_iter().map(|stub| {
                    if let ObjectMapContentItem::Map((id_str, state_id)) = stub {
                        (ObjectId::from_str(id_str.as_str()).unwrap(), state_id)
                    } else {
                        unreachable!()
                    }
                })
            )
        } else {
            let step: u32 = 10;
            let mut states = HashMap::default(); 
            loop {
                let iter = op.next(step).await?;
                let len = iter.len() as u32;
                for (contract_id, state_id) in iter.into_iter().map(|stub| {
                    if let ObjectMapContentItem::Map((id_str, state_id)) = stub {
                        (ObjectId::from_str(id_str.as_str()).unwrap(), state_id)
                    } else {
                        unreachable!()
                    }
                }) {
                    states.insert(contract_id, state_id);
                }
                if len < step {
                    break;
                }
            }
            states
        };
        Ok(states)
    }

    pub(crate) async fn query_states(&self, contracts: HashMap<ObjectId, Option<ObjectId>>) -> BuckyResult<HashMap<ObjectId, ObjectId>> {
        let mut states = HashMap::default(); 
        let op = self.stack().root_state_stub(None, None).create_path_op_env().await?;
        for (contract_id, state_id) in contracts {
            if let Some(cur_state_id) = op.get_by_key(format!("/dsg-service/contracts/{}/", contract_id), "state").await? {
                if state_id.is_none() || cur_state_id != state_id.unwrap() {
                    states.insert(contract_id, cur_state_id);
                }  
            } else {

            }
        }
        Ok(states)
    }

    async fn on_query(&self, query: DsgQuery) -> BuckyResult<DsgQuery> {
        match query {
            DsgQuery::QueryContracts {
                skip, 
                limit
            } => {
                let states = self.query_contracts(skip, limit).await?;
                Ok(DsgQuery::RespContracts { states })
            }, 
            DsgQuery::QueryStates { 
                contracts
            } => {
                let states = self.query_states(contracts).await?;
                Ok(DsgQuery::RespStates { states })
                
            }, 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid query"))
        }

    }

    pub(crate) async fn sync_contract_state(&self, state: DsgContractStateObject, from: Option<ObjectId>) -> BuckyResult<DsgContractStateObject> {
        let state_ref = DsgContractStateObjectRef::from(&state);
        let _ = self.put_object_to_noc(state_ref.id(), state_ref.as_ref()).await?;
        self.on_sync_contract_state(state, from).await
    }
    

    #[async_recursion]
    async fn on_sync_contract_state(&self, state: DsgContractStateObject, from: Option<ObjectId>) -> BuckyResult<DsgContractStateObject> {
        let state_ref = DsgContractStateObjectRef::from(&state);
        log::info!("{} on sync contract state, state={}", self, state_ref);
        let op = self.stack().root_state_stub(None, None).create_path_op_env().await
            .map_err(|err| {
                log::error!("{} on sync contract state failed, contract={}, state={}, err=operate root state {}", self, state_ref.contract_id(), state_ref.id(), err);
                err
            })?;
        let (contract, pre_state) = if let Some(pre_state_id) = state_ref.prev_state_id().cloned() {
            let contract = self.get_object_from_noc(state_ref.contract_id().clone()).await
                .map_err(|err| {
                    log::error!("{} on sync contract state failed, contract={}, state={}, err=get contract {}", self, state_ref.contract_id(), state_ref.id(), err);
                    err
                })?;
            let pre_state = self.get_object_from_noc(pre_state_id).await
                .map_err(|err| {
                    log::error!("{} on sync contract state failed, contract={}, state={}, err=get pre state {} {}", self, state_ref.contract_id(), state_ref.id(), pre_state_id, err);
                    err
                })?;
            (contract, Some(pre_state))
        } else {
            match self.get_object_from_noc(state_ref.contract_id().clone()).await {
                Ok(contract) => Ok(contract), 
                Err(err) => {
                    if BuckyErrorCode::NotFound == err.code() {
                        if let Some(from) = from {
                            log::info!("{} on sync contract state try get contract from zone, contract={}, state={}, from={}", self, state_ref.contract_id(), state_ref.id(), from);
                            self.get_object_from_device(state_ref.contract_id().clone(), from.clone()).await
                                .map_err(|err| {
                                    log::error!("{} on sync contract state failed, contract={}, state={}, err=get contract from {} {}", self, state_ref.contract_id(), state_ref.id(), from, err);
                                    err
                                })
                        } else {
                            log::error!("{} on sync contract state failed, contract={}, state={}, err=get contract {}", self, state_ref.contract_id(), state_ref.id(), err);
                            Err(err)
                        }
                    } else {
                        log::error!("{} on sync contract state failed, contract={}, state={}, err=get contract {}", self, state_ref.contract_id(), state_ref.id(), err);
                        Err(err)
                    }
                }
            }.map(|contract| (contract, None))?
        };
        
        let pre_state_ref = pre_state.as_ref().map(|state| DsgContractStateObjectRef::from(state));
        match self.on_pre_contract_state_changed(
            DsgContractObjectRef::from(&contract), 
            pre_state_ref, 
            state_ref).await {
            Ok(_) => {
                op.set_with_key(
                    format!("/dsg-service/contracts/{}/", state_ref.contract_id()), 
                    "state", 
                    &state_ref.id(), 
                    state_ref.prev_state_id().cloned(), 
                    true
                ).await.map_err(|err| {
                    log::error!("{} on sync contract state failed, contract={}, state={}, err=op root state {}", self, state_ref.contract_id(), state_ref.id(), err);
                    err
                })?;
                match op.commit().await {
                    Ok(_) => {
                        log::info!("{} contract state changed, contract={}, from={:?}, to={}", self, state_ref.contract_id(), pre_state_ref, state_ref);
                        match self.on_post_contract_state_changed(
                            DsgContractObjectRef::from(&contract), 
                            pre_state_ref, 
                            state_ref).await {
                            Ok(_) => {
                                log::info!("{} on sync contract state success, state={}", self, state_ref.id());
                                Ok(state)
                            }, 
                            Err(err) => {
                                log::error!("{} on sync contract state failed, contract={}, state={}, err=post changed {}", self, state_ref.contract_id(), state_ref.id(), err);
                                self.get_contract_state(state_ref.contract_id()).await
                            }
                        }
                    }, 
                    Err(err) => {
                        log::error!("{} on sync contract state failed, contract={}, state={}, err=op root state {}", self, state_ref.contract_id(), state_ref.id(), err);
                        self.get_contract_state(state_ref.contract_id()).await
                    }
                }
            },
            Err(err) => {
                log::error!("{} on sync contract state failed, contract={}, state={}, err=pre changed {}", self, state_ref.contract_id(), state_ref.id(), err);
                self.get_contract_state(state_ref.contract_id()).await
            }
        }
    }

    async fn on_pre_contract_state_changed<'a>(
        &self, 
        _contract: DsgContractObjectRef<'a, DsgIgnoreWitness>, 
        _from_state: Option<DsgContractStateObjectRef<'a>>, 
        _to_state: DsgContractStateObjectRef<'a>
    ) -> BuckyResult<()> {
        // FIXME: 做一些状态切换的前置检查
        Ok(())
    }


    async fn sync_data_source_to_miner<'a>(
        &self, 
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>, 
        state: DsgContractStateObjectRef<'a>, 
        prepared: &'a DsgDataSourcePreparedState
    ) -> BuckyResult<()> {
        log::info!("{} sync data source to miner, contract={}, state={}", self, contract, state);
        let challenge = match self.get_contract_latest_challenge(&contract.id()).await {
            Ok(challenge) => {
                let challenge_ref = DsgChallengeObjectRef::from(&challenge);
                log::info!("{} sync data source to miner ignored, contract={}, state={}, reason=challenge {} exists", self, contract.id(), state.id(), challenge_ref.id());
                if !state.id().eq(challenge_ref.contract_state()) {
                    // 已经不是这个challenge 了；不管了
                    Err(BuckyError::new(BuckyErrorCode::AlreadyExists, "has other state"))
                } else {
                    Ok(challenge)
                }
            }, 
            Err(err) => {
                if err.code() != BuckyErrorCode::NotFound {
                    log::error!("{} sync data source to miner failed, contract={}, state={}, reason=get latest challenge {}", self, contract.id(), state.id(), err);
                    Err(err)
                } else {
                    let challenge = self.create_challenge(state, prepared, &self.config().initial_challenge).await
                        .map_err(|err| {
                            log::error!("{} sync data source to miner failed, contract={}, state={}, reason=create challenge {}", self, contract.id(), state.id(), err);
                            err
                        })?;
                    let challenge_ref = DsgChallengeObjectRef::from(&challenge);
                    let op = self.stack().root_state_stub(None, None).create_path_op_env().await
                        .map_err(|err| {
                            log::error!("{} sync data source to miner failed, contract={}, state={}, reason=op root state {}", self, contract.id(), state.id(), err);
                            err
                        })?;
                    op.insert_with_key(format!("/dsg-service/contracts/{}/", contract.id()), "challenge", &challenge_ref.id()).await
                        .map_err(|err| {
                            log::error!("{} sync data source to miner failed, contract={}, state={}, reason=op root state {}", self, contract.id(), state.id(), err);
                            err
                        })?;
                    op.commit().await
                        .map_err(|err| {
                            log::error!("{} sync data source to miner failed, contract={}, state={}, reason=op root state {}", self, contract.id(), state.id(), err);
                            err
                        })?;
                    log::info!("{} sync data source to miner create challenge, contract={}, state={}, challenge={}", self, contract.id(), state.id(), challenge_ref);
                    Ok(challenge)
                }
            }
        }?;

        let challenge_ref = DsgChallengeObjectRef::from(&challenge);
        let _ = self.post_challenge(challenge_ref, contract.miner().clone()).await
            .map_err(|err| {
                log::error!("{} sync data source to miner failed, contract={}, state={}, challenge={}, reason=post challenge to {} {}", self, contract.id(), state.id(), challenge_ref.id(), contract.miner(), err);
                err
            })?;
        log::info!("{} post initial challenge, contract={}, challenge={}", self, contract, challenge_ref);
        let syncing = state.next(DsgContractState::DataSourceSyncing).unwrap();
        self.put_object_to_noc(DsgContractStateObjectRef::from(&syncing).id(), &syncing).await
            .map_err(|err| {
                log::error!("{} sync data source to miner failed, contract={}, state={}, challenge={}, miner={}, err=put noc {}", self, contract.id(), state.id(), challenge_ref.id(), contract.miner(), err);
                err
            })?;
        
        log::info!("{} set contract syncing data source, contract={}, state={}", self, contract, state);
        let _ = self.on_sync_contract_state(syncing, None).await?;
        Ok(())
    }

    async fn prepare_data_source<'a>(
        &self, 
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>, 
        state: DsgContractStateObjectRef<'a>, 
        changed: &'a DsgDataSourceChangedState
    ) -> BuckyResult<()> {
        log::info!("{} prepare data source, contract={}, changed={}", self, contract, state);
        // let service = self.clone();
        let sources = ChunkListDesc::from_chunks(&changed.chunks);
        let stub = match contract.storage() {
            DsgStorage::Cache(_) => {
                DsgDataSourceStubObjectRef::unchanged()
            }, 
            DsgStorage::Backup(_) => {
                DsgDataSourceStubObjectRef::merge_with_key(
                    self.stack(), 
                    sources.clone(), 
                    AesKey::random(), 
                    self.config().merge_chunk_size, 
                    self.config().split_chunk_size as u32)
            }
        };
        let stub_ref = DsgDataSourceStubObjectRef::from(&stub);
        log::info!("{} prepare data source with function, contract={}, changed={}, stub={}", self, contract.id(), state.id(), stub_ref);
       
        let to_store_chunks = stub_ref.apply(self.0.stack.clone(), sources).await
            .map_err(|err| {
                log::error!("{} prepare data source failed, contract={}, changed={}, stub={}, err=apply functions {}", self, contract.id(), state.id(), stub_ref.id(), err);
                err
            })?;
        self.put_object_to_noc(stub_ref.id(), stub_ref.as_ref()).await
            .map_err(|err| {
                log::error!("{} prepare data source failed, contract={}, changed={}, stub={}, err=put stub to noc {}", self, contract.id(), state.id(), stub_ref.id(), err);
                err
            })?;

        let prepared = state.next(DsgContractState::DataSourcePrepared(
            DsgDataSourcePreparedState {
                chunks: to_store_chunks, 
                data_source_stub: stub_ref.id()
            })).unwrap();
        self.put_object_to_noc(DsgContractStateObjectRef::from(&prepared).id(), &prepared).await
            .map_err(|err| {
                log::error!("{} prepare data source failed, contract={}, changed={}, stub={}, err=put state to noc {}", self, contract.id(), state.id(), stub_ref.id(), err);
                err
            })?;
        self.on_sync_contract_state(prepared, None).await
            .map_err(|err| {
                log::error!("{} prepare data source failed, contract={}, changed={}, stub={}, err={}", self, contract.id(), state.id(), stub_ref.id(), err);
                err
            })
            .map(|_| ())
    }

    async fn set_finish_state<'a>(
        &self, 
        state: DsgContractStateObjectRef<'a> 
    ) -> BuckyResult<()> {
        log::info!("{} try set finish state, contract={}, state={}", self, state.contract_id(), state);
        let _ = self.put_object_to_noc(state.id(), state.as_ref()).await
            .map_err(|err| {
                log::error!("{} set finish state failed, contract={}, state={}, err=put state {}", self, state.contract_id(), state.id(), err);   
                err
            })?;
        let op = self.stack().root_state_stub(None, None).create_path_op_env().await
            .map_err(|err| {
                log::error!("{} set finish state failed, contract={}, state={}, err=creat op {}", self, state.contract_id(), state.id(), err);   
                err
            })?;
        let _ = op.set_with_key(
            format!("/dsg-service/contracts/{}/", state.contract_id()), 
            "state", 
            &state.id(), 
            state.prev_state_id().cloned()
            , false).await
            .map_err(|err| {
                log::error!("{} set finish state failed, contract={}, state={}, err=set state {}", self, state.contract_id(), state.id(), err);   
                err
            })?;
        let challenge_id = op.remove_with_key(
            format!("/dsg-service/contracts/{}/", state.contract_id()), 
            "challenge", 
            None).await
            .map_err(|err| {
                log::error!("{} set finish state failed, contract={}, state={}, err=remove challenge {}", self, state.contract_id(), state.id(), err);   
                err
            })?;
        let _ = op.commit().await
            .map_err(|err| {
                log::error!("{} set finish state failed, contract={}, state={}, err=commit op {}", self, state.contract_id(), state.id(), err);   
                err
            })?;
        if let Some(challenge_id) = challenge_id {
            self.contracts().finish_challenge_id(state.contract_id(), challenge_id);
        }
        self.contracts().finish_contract(state.contract_id().clone());
        log::info!("{} set finish state, contract={}, state={}", self, state.contract_id(), state.id());
        Ok(())
    }

    async fn on_post_contract_state_changed<'a>(
        &self, 
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>, 
        _from_state: Option<DsgContractStateObjectRef<'a>>, 
        to_state: DsgContractStateObjectRef<'a>
    ) -> BuckyResult<()> {
        match to_state.state() {
            DsgContractState::DataSourceChanged(changed) => {
                let _ = self.prepare_data_source(contract, to_state, changed).await?;
            }, 
            DsgContractState::DataSourcePrepared(prepared) => {
                let _ = self.sync_data_source_to_miner(contract, to_state, prepared).await?;
            }, 
            // DsgContractState::ContractBroken => {
            //     let _ = self.sync_data_source_to_miner(contract, to_state, prepared).await?;
            // }, 
            _ => {
                // do nothing
            }
        }
        Ok(())
    }

    async fn sign_proof<'a>(
        &self,  
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>, 
        proof: DsgProofObjectRef<'a>, 
        op: &PathOpEnvStub
    ) -> BuckyResult<DsgProofObject> {
        let _ = op.remove_with_key(format!("/dsg-service/contracts/{}/", contract.id()), "challenge", Some(proof.challenge().clone())).await?;
        log::info!("{} finish challenge, contract={}, challenge={}", self, contract.id(), proof.challenge());
        let signed_proof = proof.as_ref().clone();
        let signed_proof_ref = DsgProofObjectRef::from(&signed_proof);
        let _ = self.put_object_to_noc(signed_proof_ref.id(), signed_proof_ref.as_ref()).await?;
        Ok(signed_proof)
    }

    async fn on_proof<'a>(
        &self, 
        proof: DsgProofObjectRef<'a>
    ) -> BuckyResult<DsgProofObject> {
        log::info!("{} on proof, proof={}", self, proof);
        if let Ok(signed_proof) = self.get_object_from_noc(proof.id()).await {
            // FIXME: if signed return it  
            log::info!("{} on proof signed proof exists, proof={}", self, proof.id());
            return Ok(signed_proof);
        } 
        let challenge: DsgChallengeObject = self.get_object_from_noc(proof.challenge().clone()).await
            .map_err(|err| {
                log::error!("{} on proof failed, proof={}, err=get challenge {} {} ", self, proof.id(), proof.challenge(), err);
                err
            })?;
        let challenge_ref = DsgChallengeObjectRef::from(&challenge);
        let prepared_state: DsgContractStateObject = self.get_object_from_noc(challenge_ref.contract_state().clone()).await
            .map_err(|err| {
                log::error!("{} on proof failed, proof={}, err=get state {} {} ", self, proof.id(), challenge_ref.contract_state(), err);
                err
            })?;
        let prepared_state_ref = DsgContractStateObjectRef::from(&prepared_state);
        let contract: DsgContractObject<DsgIgnoreWitness> = self.get_object_from_noc(prepared_state_ref.contract_id().clone()).await
            .map_err(|err| {
                log::error!("{} on proof failed, proof={}, err=get contract {} {} ", self, proof.id(), prepared_state_ref.contract_id(), err);
                err
            })?;
        let contract_ref = DsgContractObjectRef::from(&contract);

        let op = self.stack().root_state_stub(None, None).create_path_op_env().await
            .map_err(|err| {
                log::error!("{} on proof failed, proof={}, err=op root state {} ", self, proof.id(), err);
                err
            })?;
        let posted_challenge = op.get_by_key(format!("/dsg-service/contracts/{}/", prepared_state_ref.contract_id()), "challenge").await
            .map_err(|err| {
                log::error!("{} on proof failed, proof={}, err=op root state {} ", self, proof.id(), err);
                err
            })?
            .ok_or_else(|| {
                let err = BuckyError::new(BuckyErrorCode::ErrorState, "no challenge");
                log::error!("{} on proof failed, proof={}, err={}", self, proof.id(), err);
                err
            })?;
        if posted_challenge != challenge_ref.id() {
            let err = BuckyError::new(BuckyErrorCode::ErrorState, "mismatch challenge");
            log::error!("{} on proof failed, proof={}, err={}", self, proof.id(), err);
            Err(err)
        } else {
            //verify and sign it 
            if let DsgContractState::DataSourcePrepared(prepared) = prepared_state_ref.state() {
                let changed = self.get_object_from_noc(prepared_state_ref.prev_state_id().unwrap().clone()).await
                    .map_err(|err| {
                        log::error!("{} on proof failed, proof={}, err=get changed object {} {}", self, proof.id(), prepared_state_ref.prev_state_id().unwrap(), err);
                        err
                    })?;
                let changed_ref = DsgContractStateObjectRef::from(&changed);
                let changed = if let DsgContractState::DataSourceChanged(changed) = changed_ref.state() {
                    Ok(changed)
                } else {
                    Err(BuckyError::new(BuckyErrorCode::ErrorState, ""))
                }?;

                let stub = self.get_object_from_noc(prepared.data_source_stub.clone()).await
                    .map_err(|err| {
                        log::error!("{} on proof failed, proof={}, err=get data source stub {} {}", self, proof.id(), prepared.data_source_stub, err);
                        err
                    })?;
                let stub_ref = DsgDataSourceStubObjectRef::from(&stub);
                let sources = ChunkListDesc::from_chunks(&changed.chunks);
                let merged = ChunkListDesc::from_chunks(&prepared.chunks);
                let verified = proof.verify(self.stack(), challenge_ref, merged, sources, stub_ref, self.chunk_reader()).await
                    .map_err(|err| {
                        log::error!("{} on proof failed, proof={}, err=verify {}", self, proof.id(), err);
                        err
                    })?;
                if verified {
                    let cur_state_id = op.get_by_key(format!("/dsg-service/contracts/{}/", prepared_state_ref.contract_id()), "state").await
                        .map_err(|err| {
                            log::error!("{} on proof failed, proof={}, err=get cur state {}", self, proof.id(), err);
                            err
                        })?
                        .ok_or_else(|| {
                            log::error!("{} on proof failed, proof={}, err=get cur state no contract", self, proof.id());
                            BuckyError::new(BuckyErrorCode::ErrorState, "no contract")
                        })?;
                    let cur_state: DsgContractStateObject = self.get_object_from_noc(cur_state_id.clone()).await
                        .map_err(|err| {
                            log::error!("{} on proof failed, proof={}, err=get cur state {}", self, proof.id(), err);
                            err
                        })?;
                    let cur_state_ref = DsgContractStateObjectRef::from(&cur_state);
                    match cur_state_ref.state() {
                        DsgContractState::DataSourceSyncing => {
                            if cur_state_ref.prev_state_id().is_none() || !cur_state_ref.prev_state_id().unwrap().eq(&prepared_state_ref.id()) {
                                let err = BuckyError::new(BuckyErrorCode::ErrorState, "mismatch challenge");
                                log::error!("{} on proof failed, proof={}, err=get cur state {}", self, proof.id(), err);
                                Err(err)
                            } else {
                                let stored_state = prepared_state_ref.next(DsgContractState::DataSourceStored).unwrap();
                                let stored_state_ref = DsgContractStateObjectRef::from(&stored_state);
                                self.put_object_to_noc(stored_state_ref.id(), stored_state_ref.as_ref()).await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=put signed proof {}", self, proof.id(), err);
                                        err
                                    })?;
                                let _ = op.set_with_key(format!("/dsg-service/contracts/{}/", prepared_state_ref.contract_id()), "state", &stored_state_ref.id(), Some(cur_state_id), false).await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=set contract state {}", self, proof.id(), err);
                                        err
                                    })?;
                                let signed_proof = self.sign_proof(contract_ref.clone(), proof, &op).await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=sign {}", self, proof.id(), err);
                                        err
                                    })?;
                                let _ = op.commit().await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=commit proof {}", self, proof.id(), err);
                                        err
                                    })?;
                                self.contracts().finish_challenge(challenge_ref);
                                let _ = self.on_post_contract_state_changed(contract_ref.clone(), Some(cur_state_ref), stored_state_ref).await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=on post state changed {}", self, proof.id(), err);
                                        err
                                    })?;
                                log::info!("{} on proof set contract stored, proof={}, contract={}", self, proof.id(), contract_ref);
                                log::info!("{} on proof sign proof, proof={}", self, proof.id());
                                Ok(signed_proof)
                            }
                        }, 
                        DsgContractState::DataSourceStored => {
                            if cur_state_ref.prev_state_id().is_none() || !cur_state_ref.prev_state_id().unwrap().eq(&prepared_state_ref.id()) {
                                let err = BuckyError::new(BuckyErrorCode::ErrorState, "mismatch challenge");
                                log::error!("{} on proof failed, proof={}, err={}", self, proof.id(), err);
                                Err(err)
                            } else {
                                let signed_proof = self.sign_proof(contract_ref, proof, &op).await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=sign {}", self, proof.id(), err);
                                        err
                                    })?;
                                let _ = op.commit().await
                                    .map_err(|err| {
                                        log::error!("{} on proof failed, proof={}, err=commit op {}", self, proof.id(), err);
                                        err
                                    })?;
                                self.contracts().finish_challenge(challenge_ref);
                                log::info!("{} on proof sign proof, proof={}", self, proof.id());
                                Ok(signed_proof)
                            }
                        },
                        _ => {
                            let err = BuckyError::new(BuckyErrorCode::ErrorState, "proof in error state");
                            log::error!("{} on proof failed, proof={}, err={} {}", self, proof.id(), err, cur_state_ref);
                            Err(err)
                        }
                    }
                } else {
                    let err = BuckyError::new(BuckyErrorCode::Reject, "verify failed");
                    log::error!("{} on proof failed, proof={}, err={}", self, proof.id(), err);
                    Err(err)
                }
            } else {
                unreachable!()
            }
        }
    }


    async fn get_object_from_noc<T: for <'de> RawDecode<'de>>(&self, id: ObjectId) -> BuckyResult<T> {
        let resp = self.stack().non_service().get_object(NONGetObjectOutputRequest::new(NONAPILevel::NOC, id, None)).await?;
        T::clone_from_slice(resp.object.object_raw.as_slice())
    }

    async fn get_object_from_device<T: for <'de> RawDecode<'de>>(&self, from: ObjectId, id: ObjectId) -> BuckyResult<T> {
        let mut req = NONGetObjectOutputRequest::new(NONAPILevel::NON, id, None);
        req.common.target = Some(from);
        let resp = self.stack().non_service().get_object(req).await?;
        T::clone_from_slice(resp.object.object_raw.as_slice())
    }


    async fn put_object_to_noc<T: RawEncode>(&self, id: ObjectId, object: &T) -> BuckyResult<()> {
        let _ = self.stack().non_service().put_object(NONPutObjectOutputRequest::new(NONAPILevel::NOC, id, object.to_vec()?)).await?;
        Ok(())
    }

    async fn on_time_escape(&self, now: u64) -> BuckyResult<()> {
        let mut contracts = LinkedList::new(); 
        {
            let op = self.stack().root_state_stub(None, None).create_single_op_env().await?;
            op.load_by_path("/dsg-service/contracts/").await?;
            loop {
                let iter = op.next(1).await?;
                if iter.len() == 0 {
                    break;
                }
                if let ObjectMapContentItem::Map((id_str, _contract_state)) = &iter[0] {
                    contracts.push_back(ObjectId::from_str(id_str.as_str()).unwrap());
                } else {
                    unreachable!()
                }
            }
        }

        log::debug!("{} will check contracts {:?}", self, contracts);
        for contract_id in contracts {
            //FIXME: call parellel
            let _ = self.check_contract_state(contract_id, now).await;
        }
        Ok(())
    }

    async fn post_challenge<'a>(&self, challenge: DsgChallengeObjectRef<'a>, miner: ObjectId) -> BuckyResult<()> {
        log::debug!("{} try post challenge, challenge={}", self, challenge);   
        let mut req = NONPostObjectOutputRequest::new(NONAPILevel::default(), challenge.id(), challenge.as_ref().to_vec().unwrap());
        req.common.target = Some(miner);
        if self.contracts().pre_post_challenge(challenge, self.config()) {
            let _ = self.stack().non_service().post_object(req).await
                .map_err(|err| {
                    err
                })?;
            self.contracts().post_post_challenge(challenge);
        }
        Ok(())
    }

    async fn check_contract_state(&self, contract_id: ObjectId, now: u64) -> BuckyResult<()> {
        log::debug!("{} check contract, contract={}, at={}", self, contract_id, now);
        let op = self.stack().root_state_stub(None, None).create_single_op_env().await
            .map_err(|err| {
                log::debug!("{} check contract failed, contract={}, at={}, err= create op {}", self, contract_id, now, err);   
                err
            })?;
        op.load_by_path(format!("/dsg-service/contracts/{}/", contract_id)).await
            .map_err(|err| {
                log::debug!("{} check contract failed, contract={}, at={}, err=load path {}", self, contract_id, now, err);   
                err
            })?;
        if let Some(challenge_id) = op.get_by_key("challenge").await
            .map_err(|err| {
                log::debug!("{} check contract failed, contract={}, at={}, err=get challenge {}", self, contract_id, now, err);   
                err
            })? {
            let challenge = self.get_object_from_noc(challenge_id).await
                .map_err(|err| {
                    log::debug!("{} check contract failed, contract={}, at={}, err=load challenge {} {}", self, contract_id, now, challenge_id, err);   
                    err
                })?;
            let challenge_ref = DsgChallengeObjectRef::from(&challenge);
            log::debug!("{} check contract, contract={}, at={}, challenge={}", self, contract_id, now, challenge_ref);
            if now > challenge_ref.create_at() 
                && Duration::from_micros(now - challenge_ref.create_at()) > self.config().atomic_interval {
                let contract: DsgContractObject<DsgIgnoreWitness> = self.get_object_from_noc(contract_id.clone()).await?;
                let contract_ref = DsgContractObjectRef::from(&contract);
                if now > challenge_ref.expire_at() {
                    log::error!("{} check contract challenge expired, contract={}, at={}, challenge={}", self, contract_id, now, challenge_ref); 
                    // set to broken
                    let state_id = op.get_by_key("state").await
                        .map_err(|err| {
                            log::debug!("{} check contract failed, contract={}, at={}, err=get state {}", self, contract_id, now, err);   
                            err
                        })?
                        .ok_or_else(|| BuckyError::new(BuckyErrorCode::ErrorState, "no state"))
                        .map_err(|err| {
                            log::debug!("{} check contract failed, contract={}, at={}, err=get state {}", self, contract_id, now, err);   
                            err
                        })?;
                    let state = self.get_object_from_noc(state_id).await
                        .map_err(|err| {
                            log::debug!("{} check contract failed, contract={}, at={}, err=load state {} {}", self, contract_id, now, state_id, err);   
                            err
                        })?;
                    let state_ref = DsgContractStateObjectRef::from(&state);
                    let broken_state = match state_ref.state() {
                        DsgContractState::DataSourcePrepared(_) => {
                            state_ref.next(DsgContractState::ContractBroken).unwrap()
                        }, 
                        DsgContractState::DataSourceSyncing => {
                            state_ref.next(DsgContractState::ContractBroken).unwrap()
                        },
                        DsgContractState::DataSourceStored => {
                            state_ref.next(DsgContractState::ContractBroken).unwrap()
                        }, 
                        _ => {
                            unreachable!()
                        }
                    };
                    let broken_state_ref = DsgContractStateObjectRef::from(&broken_state);
                    self.set_finish_state(broken_state_ref).await
                } else {
                    log::debug!("{} check contract repost challenge, contract={}, at={}, challenge={}", self, contract_id, now, challenge_ref);   
                    // repost challenge to miner
                    let _ = self.post_challenge(challenge_ref, contract_ref.miner().clone()).await;
                    Ok(())
                }
            } else {
                Ok(())
            }
        } else if let Some(state_id) = op.get_by_key("state").await
            .map_err(|err| {
                log::debug!("{} check contract failed, contract={}, at={}, err=get state {}", self, contract_id, now, err);   
                err
            })? {
            let state = self.get_object_from_noc(state_id).await
                .map_err(|err| {
                    log::debug!("{} check contract failed, contract={}, at={}, err=load state {} {}", self, contract_id, now, state_id, err);   
                    err
                })?;
            let state_ref = DsgContractStateObjectRef::from(&state);
            match state_ref.state() {
                DsgContractState::DataSourceStored => {
                    if now > state_ref.create_at() {
                        let contract: DsgContractObject<DsgIgnoreWitness> = self.get_object_from_noc(contract_id.clone()).await
                            .map_err(|err| {
                                log::debug!("{} check contract failed, contract={}, at={}, err=load contract {}", self, contract_id, now, err);   
                                err
                            })?;
                        let contract_ref = DsgContractObjectRef::from(&contract);
                        if now > contract_ref.end_at() {
                            log::info!("{} check contract executed, contract={}, at={}", self, contract_id, now);   
                            let executed_state = state_ref.next(DsgContractState::ContractExecuted).unwrap();
                            let _ = self.set_finish_state(DsgContractStateObjectRef::from(&executed_state)).await;
                        } else if Duration::from_micros(now - state_ref.create_at()) > self.config().challenge_interval {
                            log::info!("{} check contract new store challenge, contract={}, at={}", self, contract_id, now);
                            let prepared_state = self.get_object_from_noc(state_ref.prev_state_id().unwrap().clone()).await
                                .map_err(|err| {
                                    log::debug!("{} check contract failed, contract={}, at={}, err=load state {} {}", self, contract_id, now, state_ref.prev_state_id().unwrap(), err);   
                                    err
                                })?;
                            let prepared_state_ref = DsgContractStateObjectRef::from(&prepared_state);
                            if let DsgContractState::DataSourcePrepared(prepared) = prepared_state_ref.state() {
                                let challenge = self.create_challenge(prepared_state_ref, prepared, &self.config().store_challenge).await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=create challenge {}", self, contract_id, now, err);   
                                        err
                                    })?;
                                let challenge_ref = DsgChallengeObjectRef::from(&challenge);
                                op.insert_with_key("challenge", &challenge_ref.id()).await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=insert challenge op {}", self, contract_id, now, err);   
                                        err
                                    })?;
                                let new_state = op.commit().await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=insert challenge op {}", self, contract_id, now, err);   
                                        err
                                    })?;

                                let path_op = self.stack().root_state_stub(None, None).create_path_op_env().await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=commit op {}", self, contract_id, now, err);   
                                        err
                                    })?;
                                let _ = path_op.set_with_path(format!("/dsg-service/contracts/{}/", contract_id), &new_state, None, false).await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=commit op {}", self, contract_id, now, err);   
                                        err
                                    })?;
                                let _ = path_op.commit().await
                                    .map_err(|err| {
                                        log::debug!("{} check contract failed, contract={}, at={}, err=commit op {}", self, contract_id, now, err);   
                                        err
                                    })?;
                                log::info!("{} check contract create store challenge, contract={}, at={}, challenge={}", self, contract_id, now, challenge_ref);
                                let _ = self.post_challenge(challenge_ref, contract_ref.miner().clone()).await;
                            } else {
                                unreachable!()
                            }
                        } 
                    } 
                }, 
                _ => {
                    
                }
            }
            Ok(())
        } else {
            unreachable!()
        }
    }

    async fn get_contract_state(&self, contract_id: &ObjectId) -> BuckyResult<DsgContractStateObject> {
        log::info!("{} get contract state, contract={}", self, contract_id);
        let op = self.stack().root_state_stub(None, None).create_single_op_env().await
            .map_err(|err| {
                log::error!("{} get contract state failed, contract={}, err=op root state {}", self, contract_id, err);
                err    
            })?;
        op.load_by_path(format!("/dsg-service/contracts/{}/", contract_id)).await
            .map_err(|err| {
                log::error!("{} get contract state failed, contract={}, err=op root state {}", self, contract_id, err);
                err    
            })?;
        if let Some(state_id) = op.get_by_key("state").await
            .map_err(|err| {
                log::error!("{} get contract state failed, contract={}, err=op root state {}", self, contract_id, err);
                err    
            })? {
            self.get_object_from_noc(state_id).await
                .map_err(|err| {
                    log::error!("{} get contract state failed, contract={}, err=get state {} {}", self, contract_id, state_id, err);
                    err    
                })
        } else {
            log::error!("{} get contract state failed, contract={}, err=no contract state", self, contract_id);
            Err(BuckyError::new(BuckyErrorCode::NotFound, "no contract state"))
        }
    }

    async fn create_challenge<'a>(
        &self, 
        state: DsgContractStateObjectRef<'a>, 
        prepared: &DsgDataSourcePreparedState, 
        options: &DsgChallengeOptions
    ) -> BuckyResult<DsgChallengeObject> {
        log::info!("{} try create challenge, state={}, options={:?}", self, state, options);
        let challenge = DsgChallengeObjectRef::new(
            self.stack().local_device_id().object_id().clone(), 
            state.contract_id().clone(),
            state.id(), 
            &prepared.chunks, 
            options);
        let challenge_ref = DsgChallengeObjectRef::from(&challenge);
        self.put_object_to_noc(challenge_ref.id(), challenge_ref.as_ref()).await
            .map_err(|err| {
                log::info!("{} create challenge failed, state={}, options={:?}, err=put to noc {}", self, state.id(), options, err);
                err
            })?;
        log::info!("{} create challenge, state={}, options={:?}, challenge={}", self, state, options, challenge_ref);
        Ok(challenge)
    }

    async fn get_contract_latest_challenge(&self, contract_id: &ObjectId) -> BuckyResult<DsgChallengeObject> {
        let op = self.stack().root_state_stub(None, None).create_single_op_env().await?;
        op.load_by_path(format!("/dsg-service/contracts/{}/", contract_id)).await?;
        if let Some(state_id) = op.get_by_key("challenge").await? {
            self.get_object_from_noc(state_id).await?
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "no challenge"))
        }
    }
}