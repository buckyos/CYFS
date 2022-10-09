use std::{convert::TryFrom, fmt::Debug, sync::Arc};
use cyfs_base::*;
use cyfs_lib::*;
use log::info;
use crate::{contracts::*, query::*};

pub struct DsgClientInterface<T>
where
    T: 'static + Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    stack: Arc<SharedCyfsStack>,
    _reserved: std::marker::PhantomData<T>,
}

impl<T> DsgClientInterface<T>
where
    T: 'static + Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug,
{
    fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self {
            stack,
            _reserved: Default::default(),
        }
    }

    fn stack(&self) -> &SharedCyfsStack {
        self.stack.as_ref()
    }

    async fn apply_contract<'a>(&self, contract: DsgContractObjectRef<'a, T>) -> BuckyResult<()> {
        log::info!("DsgClient will apply contract, contract={}", contract);
        self.put_object_to_noc(contract.id(), contract.as_ref())
            .await
            .map_err(|err| {
                log::error!(
                    "DsgClient apply contract failed, id={}, err=put to noc {}",
                    contract.id(),
                    err
                );
                err
            })?;

        log::info!("DsgClient apply contract finished, id={}", contract.id());
        Ok(())
    }

    pub async fn sync_contract_state(
        &self,
        new_state: &DsgContractStateObject,
    ) -> BuckyResult<DsgContractStateObject> {
        let state_ref = DsgContractStateObjectRef::from(new_state);
        log::info!("DsgClient try sync contract state, state={}", state_ref);

        let path = RequestGlobalStatePath::new(None, Some("/dsg/service/sync/state/")).format_string();
        log::info!("sync contract state req_path: {}", &path);
        let mut req = NONPostObjectOutputRequest::new(
            NONAPILevel::default(),
            DsgContractStateObjectRef::from(new_state).id(),
            new_state.to_vec()?,
        );
        req.common.req_path = Some(path);

        let resp = self
            .stack()
            .non_service()
            .post_object(req)
            .await
            .map_err(|err| {
                log::error!(
                    "DsgClient sync contract state failed, contract={}, state={}, err={}",
                    state_ref.contract_id(),
                    state_ref.id(),
                    err
                );
                err
            })?;
        let resp = resp.object.unwrap();
        let cur_state = DsgContractStateObject::clone_from_slice(resp.object_raw.as_slice())
            .map_err(|err| {
                log::error!("DsgClient sync contract state failed, contract={}, state={}, err=decode resp {}", state_ref.contract_id(), state_ref.id(), err);
                err
            })?;
        let cur_state_ref = DsgContractStateObjectRef::from(&cur_state);
        if cur_state_ref != state_ref {
            log::error!(
                "DsgClient sync contract state mismatch, contract={}, state={}, cur_state={}",
                state_ref.contract_id(),
                state_ref.id(),
                cur_state_ref
            );
        } else {
            log::info!(
                "DsgClient sync contract state sucess, contract={}, state={}",
                state_ref.contract_id(),
                state_ref.id()
            );
        }
        Ok(cur_state)
    }

    pub async fn query(&self, query: DsgQuery) -> BuckyResult<DsgQuery> {
        let query_obj: DsgQueryObject = query.into();

        let path = RequestGlobalStatePath::new(None, Some("/dsg/service/query/")).format_string();
        let mut req = NONPostObjectOutputRequest::new(
            NONAPILevel::default(),
            query_obj.desc().object_id(),
            query_obj.to_vec()?,
        );
        req.common.req_path = Some(path);

        let resp = self
            .stack()
            .non_service()
            .post_object(req)
            .await?;
        let resp = resp.object.unwrap();
        let resp_obj = DsgQueryObject::clone_from_slice(resp.object_raw.as_slice())?;
        DsgQuery::try_from(resp_obj)
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

    async fn put_object_to_noc<O: RawEncode>(&self, id: ObjectId, object: &O) -> BuckyResult<()> {
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

#[async_trait::async_trait]
pub trait DsgClientDelegate: Send + Sync {
    type Witness: 'static + Send + Sync + for<'de> RawDecode<'de> + RawEncode + Clone + Debug;
    fn dec_id(&self) -> &ObjectId;
    async fn add_contract(&self, id: &ObjectId) -> BuckyResult<()>;
    async fn remove_contract(&self, id: &ObjectId) -> BuckyResult<()>;
}

struct ClientImpl<D>
where
    D: 'static + DsgClientDelegate,
{
    interface: DsgClientInterface<D::Witness>,
    delegate: D,
}

pub struct DsgClient<D>
where
    D: 'static + DsgClientDelegate,
{
    inner: Arc<ClientImpl<D>>,
}

impl<D> Clone for DsgClient<D>
where
    D: 'static + DsgClientDelegate,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<D> DsgClient<D>
where
    D: 'static + DsgClientDelegate,
{
    pub fn new(stack: Arc<SharedCyfsStack>, delegate: D) -> BuckyResult<Self> {
        let client = Self {
            inner: Arc::new(ClientImpl {
                interface: DsgClientInterface::new(stack),
                delegate,
            }),
        };

        Ok(client)
    }

    pub fn interface(&self) -> &DsgClientInterface<D::Witness> {
        &self.inner.interface
    }

    pub fn delegate(&self) -> &D {
        &self.inner.delegate
    }

    pub async fn apply_contract<'a>(
        &self,
        contract: DsgContractObjectRef<'a, D::Witness>,
    ) -> BuckyResult<()> {
        let _ = self.interface().apply_contract(contract.clone()).await?;
        let _ = self.delegate().add_contract(&contract.id()).await?;
        Ok(())
    }

    pub async fn remove_contract(&self, contract: &ObjectId) -> BuckyResult<()> {
        self.delegate().remove_contract(contract).await
    }
}
