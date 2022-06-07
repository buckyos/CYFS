use crate::{StateWeakRef, StateRef, BlockDesc, ArcWeakHelper, TxLog};
use evm::backend::{Backend, Basic, ApplyBackend, Apply};
use cyfs_base::{ObjectId, ObjectDesc, CoinTokenId, BuckyResult};
use primitive_types::{U256, H256};
use crate::chain::chain_storage::{ChainStorageWeakRef, ChainStorageRef};
use cyfs_base_meta::{ViewBlockEnum, BlockDescTrait};
use cyfs_base_meta::evm_def::Log;
use crate::state_storage::State;

pub struct MetaBackend {
    ref_state: StateWeakRef,
    gas_price: u64,
    block_desc: BlockDesc,
    caller: ObjectId,
    chain_storage: Option<ChainStorageWeakRef>,

    logs: Vec<TxLog>,
    config: evm::Config,
}

impl MetaBackend {
    pub fn new(state: &StateRef, gas_price:u64, block_desc: &BlockDesc, caller: ObjectId, chain_storage: Option<&ChainStorageRef>, config: evm::Config) -> Self {
        Self {
            ref_state: StateRef::downgrade(state),
            gas_price,
            block_desc: block_desc.clone(),
            caller,
            chain_storage: chain_storage.map(|storage|ChainStorageRef::downgrade(storage)),
            logs: vec![],
            config
        }
    }
}

impl Backend for MetaBackend {
    fn gas_price(&self) -> U256 {
        U256::from(self.gas_price)
    }

    fn origin(&self) -> ObjectId {
        self.caller.clone()
    }

    fn block_coinbase(&self) -> ObjectId {
        self.block_desc.coinbase().clone()
    }

    fn block_hash(&self, number: U256) -> H256 {
        async_std::task::block_on(async {
            self.block_hash_async(number).await.unwrap_or(H256::default())
        })
    }

    fn block_number(&self) -> U256 {
        U256::from(self.block_desc.number())
    }

    fn block_timestamp(&self) -> U256 {
        U256::from(self.block_desc.create_time())
    }

    fn block_gas_limit(&self) -> U256 {
        // TODO: 这里要不要限制？
        U256::from(u64::MAX)
    }

    fn chain_id(&self) -> U256 {
        U256::zero()
    }

    fn exists(&self, address: ObjectId) -> bool {
        async_std::task::block_on(async {
            self.exists_async(address).await.unwrap_or(false)
        })
    }

    fn basic(&self, address: ObjectId) -> Basic {
        async_std::task::block_on(async {
            self.basic_async(address).await.unwrap_or(Basic::default())
        })
    }

    fn code(&self, address: ObjectId) -> Vec<u8> {
        async_std::task::block_on(async {
            self.code_async(address).await.unwrap_or(vec![])
        })
    }

    fn storage(&self, address: ObjectId, index: H256) -> H256 {
        async_std::task::block_on(async {
            self.storage_async(address, index).await.map(|ret|H256::from(ret)).unwrap_or(H256::default())
        })
    }

    fn original_storage(&self, address: ObjectId, index: H256) -> Option<H256> {
        async_std::task::block_on(async {
            self.original_storage_async(address, index).await.map(|ret|H256::from(ret))
        })
    }
}

impl MetaBackend {
    async fn block_hash_async(&self, number: U256) -> BuckyResult<H256> {
        if let Some(storage) = &self.chain_storage {
            let desc = storage.to_rc()?.block_header(ViewBlockEnum::Number(number.as_u64() as i64)).await?;
            Ok(desc.hash().into())
        } else {
            Ok(H256::default())
        }
    }

    async fn exists_async(&self, address: ObjectId) -> BuckyResult<bool> {
        self.ref_state.to_rc()?.account_exists(&address).await
    }

    async fn basic_async(&self, address: ObjectId) -> BuckyResult<Basic> {
        let balance = self.ref_state.to_rc()?.get_balance(&address, &CoinTokenId::Coin(0)).await?;
        let nonce = self.ref_state.to_rc()?.get_nonce(&address).await?;
        Ok(Basic {
            balance: balance as u64,
            nonce: nonce as u64
        })
    }

    async fn code_async(&self, address: ObjectId) -> BuckyResult<Vec<u8>> {
        self.ref_state.to_rc()?.code(&address).await
    }

    async fn storage_async(&self, address: ObjectId, index: H256) -> BuckyResult<H256> {
        self.ref_state.to_rc()?.storage(&address, &index).await
    }

    async fn original_storage_async(&self, address: ObjectId, index: H256) -> Option<H256> {
        self.storage_async(address, index).await.ok()
    }

    async fn apply_async<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool) -> BuckyResult<()>
    where
        A: IntoIterator<Item=Apply<I>>,
        I: IntoIterator<Item=(H256, H256)>,
        L: IntoIterator<Item=Log>
    {
        for apply in values {
            match apply {
                Apply::Modify {
                    address, basic, code, storage, reset_storage,
                } => {
                    let is_empty = {
                        // 修改account的balance和nonce
                        self.ref_state.to_rc()?.modify_balance(&CoinTokenId::Coin(0), &address, basic.balance as i64).await?;
                        self.ref_state.to_rc()?.set_nonce(&address, basic.nonce as i64).await?;
                        let mut empty_code = false;
                        if let Some(code) = code {
                            empty_code = code.len() == 0;
                            self.ref_state.to_rc()?.set_code(&address, code).await?;
                        }

                        if reset_storage {
                            self.ref_state.to_rc()?.reset_storage(&address).await?;
                        }

                        for (index, value) in storage {
                            if value == H256::default() {
                                self.ref_state.to_rc()?.remove_storage(&address, &index).await?;
                            } else {
                                self.ref_state.to_rc()?.set_storage(&address, &index, value).await?;
                            }
                        }

                        basic.balance == 0 &&
                            basic.nonce == 0 &&
                            empty_code
                    };

                    if is_empty && delete_empty {
                        self.ref_state.to_rc()?.delete_contract(&address).await?;
                    }
                },
                Apply::Delete {
                    address,
                } => {
                    self.ref_state.to_rc()?.delete_contract(&address).await?;
                },
            }
        }

        for log in logs {
            self.ref_state.to_rc()?.set_log(&log.address, self.block_desc.number(), &log.topics, log.data.clone()).await?;
            self.logs.push(log.into());
        }

        Ok(())
    }

    pub fn move_logs(&mut self, logs: &mut Vec<TxLog>) {
        logs.append(&mut self.logs);
    }
}

impl ApplyBackend for MetaBackend {
    fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool) where
        A: IntoIterator<Item=Apply<I>>,
        I: IntoIterator<Item=(H256, H256)>,
        L: IntoIterator<Item=Log> {
        async_std::task::block_on(async {
            // 不在backend里存储log，log存储到Receipt里
            let _ = self.apply_async(values, logs, delete_empty).await;
        })
    }
}
