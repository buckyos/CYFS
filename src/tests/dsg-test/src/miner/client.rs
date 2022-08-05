use std::{str::FromStr, sync::Arc};
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_dsg_client::*;


struct ClientImpl {
    stack: Arc<SharedCyfsStack>,
}

#[derive(Clone)]
pub struct DsgDefaultMinerClient(Arc<ClientImpl>);

impl std::fmt::Display for DsgDefaultMinerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DsgMiner")
    }
}

impl DsgDefaultMinerClient {
    pub fn dec_id() -> ObjectId {
        DecApp::generate_id(
            ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap(),
            "cyfs default miner",
        )
    }

    pub fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self(Arc::new(ClientImpl { stack }))
    }

    fn stack(&self) -> &SharedCyfsStack {
        self.0.stack.as_ref()
    }

    pub async fn order_contract<'a>(
        &self,
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>,
    ) -> BuckyResult<DsgContractObject<DsgIgnoreWitness>> {
        log::info!("{} order contract, contract = {}", self, contract);
        let mut req = NONPostObjectOutputRequest::new(
            NONAPILevel::Router,
            contract.id(),
            contract.as_ref().to_vec()?,
        );
        req.common.dec_id = Some(Self::dec_id());
        let resp = self
            .stack()
            .non_service()
            .post_object(req)
            .await
            .map_err(|err| {
                log::error!(
                    "{} order contract failed, contract={}, err={}",
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
                    "{} order contract failed, contract={}, err={}",
                    self,
                    contract.id(),
                    err
                );
                err
            })?;
        let contract = DsgContractObject::clone_from_slice(resp.object_raw.as_slice())?;
        Ok(contract)
    }

    pub async fn apply_contract<'a>(
        &self,
        contract: DsgContractObjectRef<'a, DsgIgnoreWitness>,
    ) -> BuckyResult<()> {
        log::info!("{} apply contract, contract = {}", self, contract);
        let mut req = NONPostObjectOutputRequest::new(
            NONAPILevel::Router,
            contract.id(),
            contract.as_ref().to_vec()?,
        );
        req.common.dec_id = Some(Self::dec_id());
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
        resp
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

        Ok(())
    }
}
