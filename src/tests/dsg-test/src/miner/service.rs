use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use async_std::{future, task};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;
use cyfs_dsg_client::*;
use crate::miner::*;
use super::client::DsgDefaultMinerClient;


#[derive(Clone)]
enum Prooving {
    Initial,
    Syncing(String /*task id*/),
    Signing(DsgProofObject),
}

struct MinerImpl {
    stack: Arc<SharedCyfsStack>,
    prooving: Mutex<BTreeMap<ObjectId, Prooving>>,
    config: DsgDefaultMinerConfig,
}

pub struct DsgDefaultMinerConfig {
    pub atomic_interval: Duration,
}

impl Default for DsgDefaultMinerConfig {
    fn default() -> Self {
        Self {
            atomic_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
pub struct DsgDefaultMiner(Arc<MinerImpl>);

impl DsgDefaultMiner {
    pub async fn new(
        stack: Arc<SharedCyfsStack>,
        config: DsgDefaultMinerConfig,
    ) -> BuckyResult<Self> {
        let miner = Self(Arc::new(MinerImpl {
            stack,
            prooving: Mutex::new(BTreeMap::new()),
            config,
        }));
        let _ = miner.listen()?;
        let _ = miner.load().await?;

        {
            let miner = miner.clone();
            task::spawn(async move {
                loop {
                    miner.on_time_escape(bucky_time_now());
                    let _ =
                        future::timeout(miner.config().atomic_interval, future::pending::<()>())
                            .await;
                }
            });
        }

        Ok(miner)
    }

    async fn load(&self) -> BuckyResult<()> {
        Ok(())
    }

    fn listen(&self) -> BuckyResult<()> {
        struct OnContract {
            miner: DsgDefaultMiner,
        }

        #[async_trait]
        impl
            EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
            for OnContract
        {
            async fn call(
                &self,
                param: &RouterHandlerPostObjectRequest,
            ) -> BuckyResult<RouterHandlerPostObjectResult> {
                log::info!(
                    "{} OnContract, id={}, from={}",
                    self.miner,
                    param.request.object.object_id,
                    param.request.common.source
                );
                let contract = DsgContractObject::<DsgIgnoreWitness>::clone_from_slice(
                    param.request.object.object_raw.as_slice(),
                )
                .map_err(|err| {
                    log::info!(
                        "{} OnContract failed, id={}, from={}, err=decode contract {}",
                        self.miner,
                        param.request.object.object_id,
                        param.request.common.source,
                        err
                    );
                    err
                })?;
                let contract = self.miner.on_contract(contract).await.map_err(|err| {
                    log::info!(
                        "{} OnContract failed, id={}, from={}, err=delegate {}",
                        self.miner,
                        param.request.object.object_id,
                        param.request.common.source,
                        err
                    );
                    err
                })?;
                let contract_ref = DsgContractObjectRef::from(&contract);
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(Ok(NONPostObjectInputResponse {
                        object: Some(NONObjectInfo {
                            object_id: contract_ref.id(),
                            object_raw: contract_ref.as_ref().to_vec()?,
                            object: None,
                        }),
                    })),
                })
            }
        }

        let _ = self.stack().router_handlers().add_handler(
            RouterHandlerChain::PreRouter,
            "OnContract",
            0,
            format!(
                "obj_type == {} && object.dec_id == {} && dec_id == {}",
                DsgContractDesc::<DsgIgnoreWitness>::obj_type(),
                dsg_dec_id(),
                DsgDefaultMinerClient::dec_id()
            )
            .as_str(),
            RouterHandlerAction::Default,
            Some(Box::new(OnContract {
                miner: self.clone(),
            })),
        )?;

        Ok(())
    }

    fn config(&self) -> &DsgDefaultMinerConfig {
        &self.0.config
    }

    fn stack(&self) -> &SharedCyfsStack {
        self.0.stack.as_ref()
    }

    fn prooving_of(&self, challenge: &ObjectId) -> Option<Prooving> {
        self.0.prooving.lock().unwrap().get(challenge).cloned()
    }

    async fn on_contract(
        &self,
        contract: DsgContractObject<DsgIgnoreWitness>,
    ) -> BuckyResult<DsgContractObject<DsgIgnoreWitness>> {
        let contract_ref = DsgContractObjectRef::from(&contract);
        if contract_ref.miner().eq(&ObjectId::default()) {
            //FIXME: verify space
            // let contract_ref = DsgContractObjectMutRef::from(&mut contract);
            Ok(contract)
        } else {
            //FIXME: verify ordered
            // let contract_ref = DsgContractObjectMutRef::from(&mut contract);
            Ok(contract)
        }
    }

    async fn proof_of<'a>(&self, challenge: DsgChallengeObjectRef<'a>) -> BuckyResult<ObjectId> {
        let op = self
            .stack()
            .root_state_stub(None, None)
            .create_single_op_env()
            .await
            .map_err(|err| {
                log::error!(
                    "{} get proof failed, challenge={}, err=create op {}",
                    self,
                    challenge.id(),
                    err
                );
                err
            })?;
        let _ = op
            .load_by_path(format!(
                "/dsg-miner/contracts/{}/prooves/",
                challenge.contract_id()
            ))
            .await
            .map_err(|err| {
                log::error!(
                    "{} get proof failed, challenge={}, err=load path {}",
                    self,
                    challenge.id(),
                    err
                );
                err
            })?;
        let proof_id = op
            .get_by_key(challenge.id().to_string())
            .await
            .map_err(|err| {
                log::error!(
                    "{} get proof failed, challenge={}, err=load path {}",
                    self,
                    challenge.id(),
                    err
                );
                err
            })?
            .ok_or_else(|| {
                let err = BuckyError::new(BuckyErrorCode::NotFound, "not found");
                log::error!(
                    "{} get proof failed, challenge={}, err={}",
                    self,
                    challenge.id(),
                    err
                );
                err
            })?;
        Ok(proof_id)
    }

    async fn add_proof<'a>(
        &self,
        challenge: DsgChallengeObjectRef<'a>,
        proof: DsgProofObjectRef<'a>,
    ) -> BuckyResult<()> {
        log::info!(
            "{} add proof, challenge={}, proof={}",
            self,
            challenge,
            proof
        );
        {
            let mut prooving = self.0.prooving.lock().unwrap();
            prooving.remove(&challenge.id());
        }
        let interface = DsgMinerInterface::new(self.0.stack.clone());
        interface
            .put_object_to_noc(proof.id(), proof.as_ref())
            .await
            .map_err(|err| {
                log::error!(
                    "{} add proof failed, proof={}, err=add to noc {}",
                    self,
                    proof.id(),
                    err
                );
                err
            })?;
        let op = self
            .stack()
            .root_state_stub(None, None)
            .create_path_op_env()
            .await
            .map_err(|err| {
                log::error!(
                    "{} add proof failed, proof={}, err=create op {}",
                    self,
                    proof.id(),
                    err
                );
                err
            })?;

        let _ = op
            .insert_with_key(
                format!("/dsg-miner/contracts/{}/prooves/", challenge.contract_id()),
                challenge.id().to_string(),
                &proof.id(),
            )
            .await
            .map_err(|err| {
                log::error!(
                    "{} add proof failed, proof={}, err=insert prooves {}",
                    self,
                    proof.id(),
                    err
                );
                err
            })?;

        let _ = op
            .remove_with_key(
                format!("/dsg-miner/contracts/{}/", challenge.contract_id()),
                "prooving",
                Some(challenge.id()),
            )
            .await
            .map_err(|err| {
                log::error!(
                    "{} add proof failed, proof={}, err=remove prooving {}",
                    self,
                    proof.id(),
                    err
                );
                err
            })?;
        let _ = op.commit().await.map_err(|err| {
            log::error!(
                "{} add proof failed, proof={}, err=op {}",
                self,
                proof.id(),
                err
            );
            err
        })?;
        Ok(())
    }

    async fn start_proof<'a>(&self, challenge: DsgChallengeObjectRef<'a>) -> BuckyResult<()> {
        let interface = DsgMinerInterface::new(self.0.stack.clone());
        let state = interface
            .get_object_from_noc_or_consumer(
                challenge.contract_state().clone(),
                challenge.owner().clone(),
            )
            .await
            .map_err(|err| {
                log::error!(
                    "{} start proof failed, challenge={}, err=get state {}",
                    self,
                    challenge.id(),
                    err
                );
                err
            })?;

        let state_ref = DsgContractStateObjectRef::from(&state);
        match state_ref.state() {
            DsgContractState::DataSourcePrepared(prepared) => {
                let op = self
                    .stack()
                    .root_state_stub(None, None)
                    .create_path_op_env()
                    .await
                    .map_err(|err| {
                        log::error!(
                            "{} start proof failed, challenge={}, err=create op {}",
                            self,
                            challenge.id(),
                            err
                        );
                        err
                    })?;
                let _ = op
                    .insert_with_key(
                        format!("/dsg-miner/contracts/{}/", challenge.contract_id()),
                        "prooving",
                        &challenge.id(),
                    )
                    .await
                    .map_err(|err| {
                        log::error!(
                            "{} start proof failed, challenge={}, err=op {}",
                            self,
                            challenge.id(),
                            err
                        );
                        err
                    })?;
                let _ = op.commit().await.map_err(|err| {
                    log::error!(
                        "{} start proof failed, challenge={}, err=op {}",
                        self,
                        challenge.id(),
                        err
                    );
                    err
                })?;
                let skipped = {
                    let mut prooving = self.0.prooving.lock().unwrap();
                    if prooving.get(&challenge.id()).is_some() {
                        true
                    } else {
                        prooving.insert(challenge.id(), Prooving::Initial);
                        false
                    }
                };
                if skipped {
                    log::info!("{} start proof skipped, challenge={}", self, challenge.id());
                    return Ok(());
                }

                if let Some(task_id) = interface
                    .download_chunks_from_consumer(
                        prepared.chunks.clone(),
                        challenge.owner().clone(),
                    )
                    .await?
                {
                    let mut prooving = self.0.prooving.lock().unwrap();
                    prooving.insert(challenge.id(), Prooving::Syncing(task_id.clone()));
                    log::info!(
                        "{} start proof syncing, challenge={}, task={}",
                        self,
                        challenge.id(),
                        task_id
                    );
                    Ok(())
                } else {
                    let proof = DsgProofObjectRef::proove(
                        challenge,
                        &prepared.chunks,
                        interface.chunk_reader(),
                    )
                    .await
                    .unwrap();
                    let proof_ref = DsgProofObjectRef::from(&proof);
                    if let Ok(signed) = interface
                        .verify_proof(proof_ref, challenge.owner().clone())
                        .await
                    {
                        match self
                            .add_proof(challenge, DsgProofObjectRef::from(&signed))
                            .await
                        {
                            Ok(_) => {
                                log::info!(
                                    "{} proof finished, challenge={}, proof={}",
                                    self,
                                    challenge.id(),
                                    proof_ref.id()
                                );
                            }
                            Err(_) => {
                                log::info!(
                                    "{} start proof signing, challenge={}, proof={}",
                                    self,
                                    challenge.id(),
                                    proof_ref.id()
                                );
                                let mut prooving = self.0.prooving.lock().unwrap();
                                prooving.insert(challenge.id(), Prooving::Signing(proof));
                            }
                        }
                        Ok(())
                    } else {
                        log::info!(
                            "{} start proof signing, challenge={}, proof={}",
                            self,
                            challenge.id(),
                            DsgProofObjectRef::from(&proof).id()
                        );
                        let mut prooving = self.0.prooving.lock().unwrap();
                        prooving.insert(challenge.id(), Prooving::Signing(proof));
                        Ok(())
                    }
                }
            }
            _ => {
                let err = BuckyError::new(BuckyErrorCode::ErrorState, "not in prepared state");
                log::error!(
                    "{} start proof failed, challenge={}, err={}",
                    self,
                    challenge.id(),
                    err
                );
                Err(err)
            }
        }
    }

    async fn check_prooving(
        &self,
        challenge_id: ObjectId,
        prooving: Prooving,
        _now: u64,
    ) -> BuckyResult<()> {
        let interface = DsgMinerInterface::new(self.0.stack.clone());
        match prooving {
            Prooving::Syncing(task_id) => {
                let task_state = self
                    .stack()
                    .trans()
                    .get_task_state(&TransGetTaskStateOutputRequest {
                        common: NDNOutputRequestCommon::new(NDNAPILevel::Router),
                        task_id: task_id.clone(),
                    })
                    .await?;
                let challenge = interface.get_object_from_noc(challenge_id).await?;
                let challenge_ref = DsgChallengeObjectRef::from(&challenge);
                let state = interface
                    .get_object_from_noc(challenge_ref.contract_state().clone())
                    .await?;
                match task_state {
                    TransTaskState::Finished(_) => {
                        if let DsgContractState::DataSourcePrepared(prepared) =
                            DsgContractStateObjectRef::from(&state).state()
                        {
                            let proof = DsgProofObjectRef::proove(
                                challenge_ref,
                                &prepared.chunks,
                                interface.chunk_reader(),
                            )
                            .await
                            .unwrap();
                            {
                                let mut prooving = self.0.prooving.lock().unwrap();
                                prooving.insert(challenge_id, Prooving::Signing(proof.clone()));
                            }
                            let _ = self
                                .stack()
                                .trans()
                                .delete_task(&TransTaskOutputRequest {
                                    common: NDNOutputRequestCommon::new(NDNAPILevel::Router),
                                    task_id,
                                })
                                .await;
                            let signed = interface
                                .verify_proof(
                                    DsgProofObjectRef::from(&proof),
                                    challenge_ref.owner().clone(),
                                )
                                .await?;
                            let _ = self
                                .add_proof(challenge_ref, DsgProofObjectRef::from(&signed))
                                .await?;
                            Ok(())
                        } else {
                            unreachable!()
                        }
                    }
                    TransTaskState::Err(_) => {
                        if let DsgContractState::DataSourcePrepared(prepared) =
                            DsgContractStateObjectRef::from(&state).state()
                        {
                            if let Some(task_id) = interface
                                .download_chunks_from_consumer(
                                    prepared.chunks.clone(),
                                    challenge_ref.owner().clone(),
                                )
                                .await?
                            {
                                let mut prooving = self.0.prooving.lock().unwrap();
                                prooving
                                    .insert(challenge_ref.id(), Prooving::Syncing(task_id.clone()));
                                log::info!(
                                    "{} start proof syncing, challenge={}, task={}",
                                    self,
                                    challenge_ref.id(),
                                    task_id
                                );
                            }
                            let _ = self
                                .stack()
                                .trans()
                                .delete_task(&TransTaskOutputRequest {
                                    common: NDNOutputRequestCommon::new(NDNAPILevel::Router),
                                    task_id,
                                })
                                .await;
                            Ok(())
                        } else {
                            unreachable!()
                        }
                    }
                    _ => Ok(()),
                }
            }
            Prooving::Signing(proof) => {
                let challenge = interface.get_object_from_noc(challenge_id).await?;
                let challenge_ref = DsgChallengeObjectRef::from(&challenge);
                let signed = interface
                    .verify_proof(
                        DsgProofObjectRef::from(&proof),
                        challenge_ref.owner().clone(),
                    )
                    .await?;
                let _ = self
                    .add_proof(challenge_ref, DsgProofObjectRef::from(&signed))
                    .await?;
                Ok(())
            }
            _ => unreachable!(),
        }
    }

    fn on_time_escape(&self, now: u64) {
        let proovings: Vec<(ObjectId, Prooving)> = {
            let prooving = self.0.prooving.lock().unwrap();
            prooving
                .iter()
                .filter_map(|(challenge_id, prooving)| match prooving {
                    Prooving::Syncing(_) => Some((challenge_id.clone(), prooving.clone())),
                    Prooving::Signing(_) => Some((challenge_id.clone(), prooving.clone())),
                    _ => None,
                })
                .collect()
        };

        let miner = self.clone();
        task::spawn(async move {
            let _ = futures::future::join_all(proovings.iter().map(|(challenge_id, prooving)| {
                miner.check_prooving(challenge_id.clone(), prooving.clone(), now)
            }))
            .await;
        });
    }
}

impl std::fmt::Display for DsgDefaultMiner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgMiner")
    }
}

#[async_trait]
impl DsgMinerDelegate for DsgDefaultMiner {
    async fn on_challenge(
        &self,
        interface: &DsgMinerInterface,
        challenge: DsgChallengeObject,
        from: DeviceId,
    ) -> BuckyResult<()> {
        let challenge_ref = DsgChallengeObjectRef::from(&challenge);
        log::info!("{} on challenge, challenge={}", self, challenge_ref);

        if let Some(_) = self.prooving_of(&challenge_ref.id()) {
            log::info!(
                "{} on challenge skipped, challenge={}, reason=prooving",
                self,
                challenge_ref.id()
            );
            return Ok(());
        }

        if let Ok(proof_id) = self.proof_of(challenge_ref).await {
            log::info!(
                "{} on challenge skipped, challenge={}, reason=prooved",
                self,
                challenge_ref.id()
            );

            let interface = interface.clone();
            let miner = self.clone();
            let challenge_id = challenge_ref.id();
            task::spawn(async move {
                log::info!(
                    "{} on challenge verify proof, challenge={}",
                    miner,
                    challenge_id
                );
                if let Ok(proof) = interface
                    .get_object_from_noc::<DsgProofObject>(proof_id)
                    .await
                {
                    match interface
                        .verify_proof(DsgProofObjectRef::from(&proof), from.object_id().clone())
                        .await
                    {
                        Ok(_) => {
                            log::info!(
                                "{} on challenge verify proof success, challenge={}",
                                miner,
                                challenge_id
                            );
                        }
                        Err(err) => {
                            log::error!("{} on challenge verify proof failed, challenge={}, err=verify proof failed {}", miner, challenge_id, err);
                        }
                    }
                } else {
                    log::error!(
                        "{} on challenge verify proof failed, challenge={}, err=get proof failed",
                        miner,
                        challenge_id
                    );
                }
            });

            return Ok(());
        }

        let _ = interface
            .put_object_to_noc(challenge_ref.id(), challenge_ref.as_ref())
            .await
            .map_err(|err| {
                log::error!(
                    "{} on challenge failed, challenge={}, err=put challenge to noc {}",
                    self,
                    challenge_ref.id(),
                    err
                );
                err
            })?;

        let _ = self.start_proof(challenge_ref).await.map_err(|err| {
            log::error!(
                "{} on challenge failed, challenge={}, err=start proof {}",
                self,
                challenge_ref.id(),
                err
            );
            err
        })?;

        log::info!(
            "{} on challenge success, challenge={}",
            self,
            challenge_ref.id()
        );

        Ok(())
    }
}
