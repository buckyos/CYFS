use std::{str::FromStr, sync::Arc, time::Duration};
use async_std::task;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_dsg_client::*;
use dsg_service::*;
use super::miner::*;



pub struct AllInOneConsumer {
    stack: Arc<SharedCyfsStack>,
    client: DsgClient<TestClient>,
    service: DsgService,
    miner: ObjectId,
}

impl AllInOneConsumer {
    pub async fn new(miner: ObjectId) -> BuckyResult<Self> {
        let dec_id = DecApp::generate_id(
            ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap(),
            "dsg all in one",
        );
    
        let stack = Arc::new(SharedCyfsStack::open_default(Some(dec_id)).await.unwrap());
    
        let client = DsgClient::new(stack.clone(), TestClient::new(stack.clone()))?;
    
        let service = DsgService::new(stack.clone(), DsgServiceConfig::default()).await?;
    
        Ok(Self {
            stack,
            client,
            service,
            miner,
        })
    }
    
    pub fn client(&self) -> &DsgClient<TestClient> {
        &self.client
    }
    
    pub fn stack(&self) -> &SharedCyfsStack {
        self.stack.as_ref()
    }
}


pub struct AllInOneDsg {
    stack: Arc<SharedCyfsStack>,
    client: DsgClient<TestClient>,
    service: DsgService,
    miner: DsgMiner<TestMiner>,
}

impl AllInOneDsg {
    pub async fn new(service_config: Option<DsgServiceConfig>) -> BuckyResult<Self> {
        let dec_id = DecApp::generate_id(
            ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap(),
            "dsg all in one",
        );

        let stack = Arc::new(SharedCyfsStack::open_default(Some(dec_id)).await.unwrap());

        let client = DsgClient::new(stack.clone(), TestClient::new(stack.clone()))?;

        let service = DsgService::new(stack.clone(), service_config.unwrap_or_default()).await?;

        let miner = DsgMiner::new(stack.clone(), TestMiner::new()).await?;

        Ok(Self {
            stack,
            client,
            service,
            miner,
        })
    }

    pub fn client(&self) -> &DsgClient<TestClient> {
        &self.client
    }

    pub fn stack(&self) -> &SharedCyfsStack {
        self.stack.as_ref()
    }
}

pub struct TestClient {
    stack: Arc<SharedCyfsStack>,
}

impl TestClient {
    fn new(stack: Arc<SharedCyfsStack>) -> Self {
        Self { stack }
    }
}

#[async_trait]
impl DsgClientDelegate for TestClient {
    type Witness = DsgNonWitness;

    fn dec_id(&self) -> &ObjectId {
        self.stack.dec_id().unwrap()
    }

    // 如果上层应用没有在自己的rootstate里面引用contract，需要显示的 添加 和 移除 创建出来的contract object
    async fn add_contract(&self, id: &ObjectId) -> BuckyResult<()> {
        let op = self
            .stack
            .root_state_stub(None, None)
            .create_path_op_env()
            .await?;
        op.insert("/dsg-client/contracts/", id).await?;
        let _ = op.commit().await?;
        Ok(())
    }

    // 如果上层应用没有在自己的rootstate里面引用contract，需要显示的 添加 和 移除 创建出来的contract object
    async fn remove_contract(&self, id: &ObjectId) -> BuckyResult<()> {
        let op = self
            .stack
            .root_state_stub(None, None)
            .create_single_op_env()
            .await?;
        if let Err(err) = op.load_by_path("/dsg-client/contracts/").await {
            if err.code() == BuckyErrorCode::NotFound {
                Ok(())
            } else {
                Err(err)
            }
        } else {
            op.remove(id).await?;
            let _ = op.commit().await?;
            Ok(())
        }
    }
}

struct TestMiner {}

impl TestMiner {
    fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl DsgMinerDelegate for TestMiner {
    async fn on_challenge(
        &self,
        interface: &DsgMinerInterface,
        challenge: DsgChallengeObject,
        from: DeviceId,
    ) -> BuckyResult<()> {
        log::info!(
            "DsgMiner on challenge, challenge={}",
            DsgChallengeObjectRef::from(&challenge)
        );
        assert_eq!(from, interface.stack().local_device_id());

        let interface = interface.clone();
        task::spawn(async move {
            task::sleep(Duration::from_secs(1)).await;
            let challenge_ref = DsgChallengeObjectRef::from(&challenge);
            log::info!(
                "DsgMiner will proove challenge, challenge={}",
                challenge_ref
            );
            let state: DsgContractStateObject = interface
                .get_object_from_noc(challenge_ref.contract_state().clone())
                .await
                .unwrap();
            let state_ref = DsgContractStateObjectRef::from(&state);
            if let DsgContractState::DataSourcePrepared(prepared) = state_ref.state() {
                let proof = DsgProofObjectRef::proove(
                    challenge_ref,
                    &prepared.chunks,
                    interface.chunk_reader(),
                )
                .await
                .unwrap();
                let _ = interface
                    .verify_proof(DsgProofObjectRef::from(&proof), from.object_id().clone())
                    .await;
            } else {
                unreachable!()
            }
        });

        Ok(())
    }
}
