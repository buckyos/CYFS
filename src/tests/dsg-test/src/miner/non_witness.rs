use std::sync::Arc;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;
use cyfs_dsg_client::*;
use super::client::DsgDefaultMinerClient;



struct ClientImpl<D: 'static + DsgClientDelegate<Witness = DsgNonWitness>> {
    stack: Arc<SharedCyfsStack>,
    client: DsgClient<D>,
}

pub struct DsgNonWitnessClient<D: 'static + DsgClientDelegate<Witness = DsgNonWitness>>(
    Arc<ClientImpl<D>>,
);

impl<D: 'static + DsgClientDelegate<Witness = DsgNonWitness>> Clone for DsgNonWitnessClient<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<D: 'static + DsgClientDelegate<Witness = DsgNonWitness>> std::fmt::Display
    for DsgNonWitnessClient<D>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgNonWitness")
    }
}

impl<D: 'static + DsgClientDelegate<Witness = DsgNonWitness>> DsgNonWitnessClient<D> {
    pub fn new(stack: Arc<SharedCyfsStack>, delegate: D) -> BuckyResult<Self> {
        Ok(Self(Arc::new(ClientImpl {
            client: DsgClient::new(stack.clone(), delegate)?,
            stack,
        })))
    }

    fn stack(&self) -> &SharedCyfsStack {
        self.0.stack.as_ref()
    }

    fn client(&self) -> &DsgClient<D> {
        &self.0.client
    }

    pub fn interface(&self) -> &DsgClientInterface<DsgNonWitness> {
        self.client().interface()
    }

    pub async fn order_contract<'a>(
        &self,
        to: ObjectId,
        order: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<DsgContractObject<DsgNonWitness>> {
        log::info!("{} order contract, order = {}, to = {}", self, order, to);
        let req = CryptoSignObjectOutputRequest::new(
            order.id(),
            order.as_ref().to_vec()?,
            CRYPTO_REQUEST_FLAG_SIGN_SET_DESC,
        );
        let resp = self.stack().crypto_service().sign_object(req).await?;
        assert_eq!(resp.result, SignObjectResult::Signed);
        let signed = resp.object.unwrap();

        let mut req =
            NONPostObjectOutputRequest::new(NONAPILevel::Router, order.id(), signed.object_raw);
        req.common.dec_id = Some(self.client().delegate().dec_id().clone());
        req.common.target = Some(to.clone());
        let resp = self
            .stack()
            .non_service()
            .post_object(req)
            .await
            .map_err(|err| {
                log::error!(
                    "{} order contract failed, order={}, to={}, err={}",
                    self,
                    order.id(),
                    to,
                    err
                );
                err
            })?;
        let resp = resp
            .object
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::Failed, "failed"))
            .map_err(|err| {
                log::error!(
                    "{} order contract failed, order={}, to={}, err={}",
                    self,
                    order.id(),
                    to,
                    err
                );
                err
            })?;
        let contract = DsgContractObject::clone_from_slice(resp.object_raw.as_slice())?;
        let contract_ref = DsgContractObjectRef::from(&contract);

        let miner: Device = self
            .interface()
            .get_object_from_noc(contract_ref.miner().clone())
            .await?;
        let vierifier = RsaCPUObjectVerifier::new(miner.desc().public_key().clone());
        let sign = contract_ref
            .miner_signature()
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::PermissionDenied, "no miner sign"))?;
        if vierifier
            .verify(contract_ref.as_ref().desc().to_vec()?.as_slice(), sign)
            .await
        {
            Err(BuckyError::new(
                BuckyErrorCode::PermissionDenied,
                "no miner sign",
            ))
        } else {
            Ok(contract)
        }
    }

    pub async fn apply_contract<'a>(
        &self,
        contract: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<()> {
        log::info!("{} apply contract, contract = {}", self, contract);
        let req = CryptoSignObjectOutputRequest::new(
            contract.id(),
            contract.as_ref().to_vec()?,
            CRYPTO_REQUEST_FLAG_SIGN_BY_DEVICE | CRYPTO_REQUEST_FLAG_SIGN_SET_DESC,
        );
        let resp = self.stack().crypto_service().sign_object(req).await?;
        assert_eq!(resp.result, SignObjectResult::Signed);
        let signed = resp.object.unwrap();

        let mut req =
            NONPostObjectOutputRequest::new(NONAPILevel::Router, contract.id(), signed.object_raw);
        req.common.dec_id = Some(self.client().delegate().dec_id().clone());
        req.common.target = Some(contract.miner().clone());

        let resp = self
            .stack()
            .non_service()
            .post_object(req)
            .await
            .map_err(|err| {
                log::error!(
                    "{} apply contract failed, contract={}, err={}",
                    self,
                    contract.id(),
                    err
                );
                err
            })?;
        let resp = resp
            .object
            .ok_or_else(|| BuckyError::new(BuckyErrorCode::Failed, "failed"))
            .map_err(|err| {
                log::error!(
                    "{} apply contract failed, contract={}, err={}",
                    self,
                    contract.id(),
                    err
                );
                err
            })?;
        let contract = DsgContractObject::clone_from_slice(resp.object_raw.as_slice())?;

        let _ = self
            .client()
            .apply_contract(DsgContractObjectRef::from(&contract))
            .await?;
        Ok(())
    }
}

#[async_trait]
pub trait DsgNonWitnessDelegate: Send + Sync {
    fn dec_id(&self) -> &ObjectId;
    async fn on_pre_order<'a>(
        &self,
        contract: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<()>;
    async fn on_post_order<'a>(
        &self,
        result: BuckyResult<DsgContractObjectRef<'a, DsgNonWitness>>,
    ) -> BuckyResult<()>;
    async fn on_pre_apply<'a>(
        &self,
        contract: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<()>;
    async fn on_post_apply<'a>(
        &self,
        result: BuckyResult<DsgContractObjectRef<'a, DsgNonWitness>>,
    ) -> BuckyResult<()>;
}

struct ServiceImpl<T: 'static + DsgNonWitnessDelegate> {
    stack: Arc<SharedCyfsStack>,
    miner_client: DsgDefaultMinerClient,
    delegate: T,
}

pub struct DsgNonWitnessService<T: 'static + DsgNonWitnessDelegate>(Arc<ServiceImpl<T>>);

impl<T: 'static + DsgNonWitnessDelegate> DsgNonWitnessService<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: 'static + DsgNonWitnessDelegate> std::fmt::Display for DsgNonWitnessService<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgNonWitness")
    }
}

struct OnContract<T: 'static + DsgNonWitnessDelegate> {
    service: DsgNonWitnessService<T>,
}

#[async_trait]
impl<T: 'static + DsgNonWitnessDelegate>
    EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnContract<T>
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        log::info!(
            "{} OnContract, id={}, from={}",
            self.service,
            param.request.object.object_id,
            param.request.common.source
        );
        let contract = DsgContractObject::<DsgNonWitness>::clone_from_slice(
            param.request.object.object_raw.as_slice(),
        )
        .map_err(|err| {
            log::info!(
                "{} OnContract failed, id={}, from={}, err=decode contract {}",
                self.service,
                param.request.object.object_id,
                param.request.common.source,
                err
            );
            err
        })?;
        let contract = self.service.on_contract(contract).await.map_err(|err| {
            log::info!(
                "{} OnContract failed, id={}, from={}, err=delegate {}",
                self.service,
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

impl<T: 'static + DsgNonWitnessDelegate> DsgNonWitnessService<T> {
    pub async fn new(stack: Arc<SharedCyfsStack>, delegate: T) -> BuckyResult<Self> {
        let client = Self(Arc::new(ServiceImpl {
            miner_client: DsgDefaultMinerClient::new(stack.clone()),
            stack,
            delegate,
        }));
        let _ = client.listen();

        Ok(client)
    }

    fn stack(&self) -> &SharedCyfsStack {
        self.0.stack.as_ref()
    }

    fn miner_client(&self) -> &DsgDefaultMinerClient {
        &self.0.miner_client
    }

    fn delegate(&self) -> &T {
        &self.0.delegate
    }

    fn listen(&self) -> BuckyResult<()> {
        let _ = self.stack().router_handlers().add_handler(
            RouterHandlerChain::PreRouter,
            format!("OnContractFrom{}", self.delegate().dec_id()).as_str(),
            0,
            format!(
                "obj_type == {} && object.dec_id == {} && dec_id == {}",
                DsgContractDesc::<DsgNonWitness>::obj_type(),
                dsg_dec_id(),
                self.delegate().dec_id()
            )
            .as_str(),
            RouterHandlerAction::Default,
            Some(Box::new(OnContract {
                service: self.clone(),
            })),
        )?;

        Ok(())
    }

    async fn on_contract(
        &self,
        contract: DsgContractObject<DsgNonWitness>,
    ) -> BuckyResult<DsgContractObject<DsgNonWitness>> {
        let contract_ref: DsgContractObjectRef<DsgNonWitness> =
            DsgContractObjectRef::from(&contract);
        log::info!("{} on contract, contract={}", self, contract_ref);
        if contract_ref.is_order() {
            let _ = self.delegate().on_pre_order(contract_ref.clone()).await?;
            let ignore: DsgContractObject<DsgIgnoreWitness> = contract_ref.into();
            let ignore_ref = DsgContractObjectRef::from(&ignore);
            let result = self
                .miner_client()
                .order_contract(ignore_ref)
                .await
                .map(|contract| DsgContractObjectRef::from(&contract).into());
            let _ = self
                .delegate()
                .on_post_order(
                    result
                        .as_ref()
                        .map(|contract| DsgContractObjectRef::from(contract))
                        .map_err(|err| err.clone()),
                )
                .await;
            let contract = result?;
            Ok(contract)
        } else {
            let _ = self.delegate().on_pre_apply(contract_ref.clone()).await?;
            let ignore: DsgContractObject<DsgIgnoreWitness> = contract_ref.clone().into();
            let result = self
                .miner_client()
                .apply_contract(DsgContractObjectRef::from(&ignore))
                .await;
            let _ = self
                .delegate()
                .on_post_order(
                    result
                        .as_ref()
                        .map(|_| contract_ref)
                        .map_err(|err| err.clone()),
                )
                .await;
            Ok(contract)
        }
    }
}
