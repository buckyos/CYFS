use std::{convert::TryFrom, path::PathBuf, sync::Arc};
use futures::future;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_bdt::*;
use cyfs_lib::*;
use cyfs_util::*;
use cyfs_dsg_client::*;


struct InterfaceImpl {
    stack: Arc<SharedCyfsStack>,
}

#[derive(Clone)]
pub struct DsgMinerInterface {
    inner: Arc<InterfaceImpl>,
}

impl DsgMinerInterface {
    pub fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self {
            inner: Arc::new(InterfaceImpl { stack }),
        }
    }

    pub fn stack(&self) -> &SharedCyfsStack {
        self.inner.stack.as_ref()
    }

    pub fn chunk_reader(&self) -> Box<dyn ChunkReader> {
        DsgStackChunkReader::new(self.inner.stack.clone()).clone_as_reader()
    }

    pub async fn verify_proof<'a>(
        &self,
        proof: DsgProofObjectRef<'a>,
        to: ObjectId,
    ) -> BuckyResult<DsgProofObject> {
        log::info!(
            "DsgMiner will request sign for proof, proof={}, to={}",
            proof,
            to
        );
        let mut req = NONPostObjectOutputRequest::new(
            NONAPILevel::default(),
            proof.id(),
            proof.as_ref().to_vec().unwrap(),
        );
        req.common.target = Some(to.clone());
        let resp = self.stack().non_service().post_object(req).await
            .map_err(|err| {
                log::error!("DsgMiner will request sign for proof failed, proof={}, to={}, err=post object {}", proof.id(), to, err);
                err
            })?;

        if let Some(object_raw) = resp.object.as_ref().map(|o| o.object_raw.as_slice()) {
            let signed_proof = DsgProofObject::clone_from_slice(object_raw).map_err(|err| {
                log::error!(
                    "DsgMiner request sign for proof failed, proof={}, to={}, err=decode resp {}",
                    proof.id(),
                    to,
                    err
                );
                err
            })?;

            //FIXME: verify sign
            log::info!(
                "DsgMiner request sign for proof success, proof={}, to={}",
                proof.id(),
                to
            );
            Ok(signed_proof)
        } else {
            let err = BuckyError::new(BuckyErrorCode::InvalidData, "consumer return no object");
            log::error!(
                "DsgMiner request sign for proof failed, proof={}, to={}, err=decode resp {}",
                proof.id(),
                to,
                err
            );
            Err(err)
        }
    }

    pub async fn download_chunks_from_consumer(
        &self,
        chunks: Vec<ChunkId>,
        consumer: ObjectId,
    ) -> BuckyResult<Option<String>> {
        let reader = self.chunk_reader();
        let exists = future::join_all(chunks.iter().map(|chunk| reader.exists(chunk))).await;
        if exists.into_iter().fold(true, |a, b| a & b) {
            Ok(None)
        } else {
            // FIXME: chunk list task
            let bundle = ChunkBundle::new(chunks, ChunkBundleHashMethod::Serial);
            let bundle_obj = File::new_no_owner(
                bundle.len(),
                bundle.calc_hash_value(),
                ChunkList::ChunkInBundle(bundle),
            )
            .build();
            let _ = self
                .put_object_to_noc(bundle_obj.desc().object_id(), &bundle_obj)
                .await?;
            let mut req = TransCreateTaskOutputRequest {
                common: NDNOutputRequestCommon::new(NDNAPILevel::Router),
                object_id: bundle_obj.desc().object_id(),
                local_path: PathBuf::from(""),
                device_list: vec![DeviceId::try_from(&consumer)?],
                context_id: None,
                auto_start: true,
            };
            req.common.dec_id = Some(dsg_dec_id());
            let resp = self.stack().trans().create_task(&req).await?;
            Ok(Some(resp.task_id))
        }
    }

    pub async fn get_object_from_noc_or_consumer<O: RawEncode + for<'de> RawDecode<'de>>(
        &self,
        id: ObjectId,
        consumer: ObjectId,
    ) -> BuckyResult<O> {
        match self.get_object_from_noc(id.clone()).await {
            Ok(o) => Ok(o),
            Err(err) => match err.code() {
                BuckyErrorCode::NotFound => {
                    let obj = self.get_object_from_consumer(id, consumer).await?;
                    let _ = self.put_object_to_noc(id, &obj).await;
                    Ok(obj)
                }
                _ => Err(err),
            },
        }
    }

    pub async fn get_object_from_consumer<O: for<'de> RawDecode<'de>>(
        &self,
        id: ObjectId,
        consumer: ObjectId,
    ) -> BuckyResult<O> {
        let mut req = NONGetObjectOutputRequest::new(NONAPILevel::Router, id, None);
        req.common.dec_id = Some(dsg_dec_id());
        req.common.target = Some(consumer);
        let resp = self.stack().non_service().get_object(req).await?;
        O::clone_from_slice(resp.object.object_raw.as_slice())
    }

    pub async fn get_object_from_noc<O: for<'de> RawDecode<'de>>(
        &self,
        id: ObjectId,
    ) -> BuckyResult<O> {
        let resp = self
            .stack()
            .non_service()
            .get_object(NONGetObjectOutputRequest::new(NONAPILevel::NOC, id, None))
            .await?;
        O::clone_from_slice(resp.object.object_raw.as_slice())
    }

    pub async fn put_object_to_noc<O: RawEncode>(
        &self,
        id: ObjectId,
        object: &O,
    ) -> BuckyResult<()> {
        let _ = self
            .stack()
            .non_service()
            .put_object(NONPutObjectOutputRequest::new(
                NONAPILevel::NOC,
                id,
                object.to_vec()?,
            ))
            .await?;
        Ok(())
    }
}

#[async_trait]
pub trait DsgMinerDelegate: Send + Sync {
    async fn on_challenge(
        &self,
        interface: &DsgMinerInterface,
        challenge: DsgChallengeObject,
        from: DeviceId,
    ) -> BuckyResult<()>;
}

struct MinerImpl<D>
where
    D: 'static + DsgMinerDelegate,
{
    interface: DsgMinerInterface,
    delegate: D,
}

pub struct DsgMiner<D>
where
    D: 'static + DsgMinerDelegate,
{
    inner: Arc<MinerImpl<D>>,
}

impl<D> Clone for DsgMiner<D>
where
    D: 'static + DsgMinerDelegate,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<D> std::fmt::Display for DsgMiner<D>
where
    D: 'static + DsgMinerDelegate,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgMiner")
    }
}

impl<D> DsgMiner<D>
where
    D: 'static + DsgMinerDelegate,
{
    pub async fn new(stack: Arc<SharedCyfsStack>, delegate: D) -> BuckyResult<Self> {
        let miner = Self {
            inner: Arc::new(MinerImpl {
                interface: DsgMinerInterface::new(stack),
                delegate,
            }),
        };
        let _ = miner.interface().stack().wait_online(None).await?;
        let _ = miner.listen()?;
        Ok(miner)
    }

    fn interface(&self) -> &DsgMinerInterface {
        &self.inner.interface
    }

    fn delegate(&self) -> &D {
        &self.inner.delegate
    }

    fn listen(&self) -> BuckyResult<()> {
        struct OnChallenge<D>
        where
            D: 'static + DsgMinerDelegate,
        {
            miner: DsgMiner<D>,
        }

        #[async_trait]
        impl<D>
            EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
            for OnChallenge<D>
        where
            D: 'static + DsgMinerDelegate,
        {
            async fn call(
                &self,
                param: &RouterHandlerPostObjectRequest,
            ) -> BuckyResult<RouterHandlerPostObjectResult> {
                log::info!(
                    "{} OnChallenge, id={}, from={}",
                    self.miner,
                    param.request.object.object_id,
                    param.request.common.source
                );
                let challenge = DsgChallengeObject::clone_from_slice(
                    param.request.object.object_raw.as_slice(),
                )
                .map_err(|err| {
                    log::info!(
                        "{} OnChallenge failed, id={}, from={}, err=decode challenge {}",
                        self.miner,
                        param.request.object.object_id,
                        param.request.common.source,
                        err
                    );
                    err
                })?;
                let _ = self
                    .miner
                    .delegate()
                    .on_challenge(
                        self.miner.interface(),
                        challenge,
                        param.request.common.source.clone(),
                    )
                    .await
                    .map_err(|err| {
                        log::info!(
                            "{} OnChallenge failed, id={}, from={}, err=delegate {}",
                            self.miner,
                            param.request.object.object_id,
                            param.request.common.source,
                            err
                        );
                        err
                    })?;
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(Ok(NONPostObjectInputResponse { object: None })),
                })
            }
        }

        let _ = self.interface().stack().router_handlers().add_handler(
            RouterHandlerChain::PreRouter,
            "OnChallenge",
            0,
            format!("obj_type == {}", DsgChallengeDesc::obj_type()).as_str(),
            RouterHandlerAction::Default,
            Some(Box::new(OnChallenge {
                miner: self.clone(),
            })),
        )?;


        struct OnInterest<D>
        where
            D: 'static + DsgMinerDelegate,
        {
            miner: DsgMiner<D>,
        }

        #[async_trait]
        impl<D> EventListenerAsyncRoutine<RouterHandlerInterestRequest, RouterHandlerInterestResult>
            for OnInterest<D>
        where
            D: 'static + DsgMinerDelegate,
        {
            async fn call(
                &self,
                param: &RouterHandlerInterestRequest,
            ) -> BuckyResult<RouterHandlerInterestResult> {
                log::info!(
                    "{} OnInterest, interest={:?}, from={}",
                    self.miner,
                    param.request.interest, 
                    param.request.from_channel
                );
                let referer = BdtDataRefererInfo::decode_string(param.request.interest.referer.as_ref().unwrap().as_str())?;
                let contract_id = referer.referer_object[0].target.clone().unwrap();


                Ok(RouterHandlerInterestResult {
                    action: RouterHandlerAction::Default,
                    request: None,
                    response: None,
                })
            }
        }


        
        let _ = self.interface().stack().router_handlers().add_handler(
            RouterHandlerChain::NDN,
            "OnInterest",
            0,
            format!(
                "interest.referer=='*:*'",
            )
            .as_str(),
            RouterHandlerAction::Default,
            Some(Box::new(OnInterest {
                miner: self.clone(),
            })),
        )?;


        Ok(())
    }
}
