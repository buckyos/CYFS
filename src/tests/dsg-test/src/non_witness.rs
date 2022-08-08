use std::{str::FromStr, sync::Arc};
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_dsg_client::*;
use dsg_service::*;
use super::miner::*;


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

pub struct AllInOneConsumer {
    stack: Arc<SharedCyfsStack>,
    client: DsgNonWitnessClient<TestClient>,
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

        let client = DsgNonWitnessClient::new(stack.clone(), TestClient::new(stack.clone()))?;

        let service = DsgService::new(stack.clone(), DsgServiceConfig::default()).await?;

        Ok(Self {
            stack,
            client,
            service,
            miner,
        })
    }

    pub fn client(&self) -> &DsgNonWitnessClient<TestClient> {
        &self.client
    }

    pub fn stack(&self) -> &SharedCyfsStack {
        self.stack.as_ref()
    }
}

struct TestWitness {
    stack: Arc<SharedCyfsStack>,
}

#[async_trait]
impl DsgNonWitnessDelegate for TestWitness {
    fn dec_id(&self) -> &ObjectId {
        self.stack.dec_id().unwrap()
    }

    async fn on_pre_order<'a>(
        &self,
        _contract: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<()> {
        Ok(())
    }
    async fn on_post_order<'a>(
        &self,
        _result: BuckyResult<DsgContractObjectRef<'a, DsgNonWitness>>,
    ) -> BuckyResult<()> {
        Ok(())
    }
    async fn on_pre_apply<'a>(
        &self,
        _contract: DsgContractObjectRef<'a, DsgNonWitness>,
    ) -> BuckyResult<()> {
        Ok(())
    }
    async fn on_post_apply<'a>(
        &self,
        _result: BuckyResult<DsgContractObjectRef<'a, DsgNonWitness>>,
    ) -> BuckyResult<()> {
        Ok(())
    }
}

pub struct AllInOneMiner {
    stack: Arc<SharedCyfsStack>,
    witness: DsgNonWitnessService<TestWitness>,
    service: DsgMiner<DsgDefaultMiner>,
}

impl AllInOneMiner {
    pub async fn new() -> BuckyResult<Self> {
        let dec_id = DecApp::generate_id(
            ObjectId::from_str("5r4MYfFPKMeHa1fec7dHKmBfowySBfVFvRQvKB956dnF").unwrap(),
            "dsg all in one",
        );

        let stack = Arc::new(SharedCyfsStack::open_default(Some(dec_id)).await.unwrap());

        let witness = DsgNonWitnessService::new(
            stack.clone(),
            TestWitness {
                stack: stack.clone(),
            },
        )
        .await
        .unwrap();

        let service = DsgMiner::new(
            stack.clone(),
            DsgDefaultMiner::new(stack.clone(), DsgDefaultMinerConfig::default()).await?,
        )
        .await?;

        Ok(Self {
            stack,
            witness,
            service,
        })
    }
}
